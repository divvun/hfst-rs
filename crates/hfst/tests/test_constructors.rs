// Port of test/libhfst/test_constructors.cc
//
// Tests HfstTransducer constructors, destructor, operator=, and the member
// functions set_name, get_name and get_type.
//
// The C++ main loops over the implementation types {SFST, TROPICAL, FOMA}
// (with LOG commented out). Per the Wave-2 port scope, only the in-scope
// OpenFST backend is exercised here; with the monomorphic backends the loop
// body becomes helpers generic over the backend type, instantiated for the
// formerly-exercised TROPICAL_OPENFST_TYPE -> StdVectorFst. The out-of-scope
// SFST_TYPE / FOMA_TYPE / XFSM_TYPE iterations are intentionally skipped. The
// fixed HFST_OL_TYPE / HFST_OLW_TYPE usages inside the loop body (the
// operator= block) are ported faithfully.
//
// Each logical group from the C++ loop body (delimited there by verbose_print
// labels) becomes its own helper, run once per in-scope type. C++
// compare(another) defaults to harmonize=true, mirrored here by compare_default.

use hfst::backend::AlgebraBackend;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{AnyTransducer, FromAnyTransducer, HfstTransducer};
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

// The tropical transition-data symbol coding lives in process-global
// statics (NUMBER2SYMBOL_MAP / SYMBOL2NUMBER_MAP / MAX_NUMBER, each behind its
// own Mutex). get_number bumps MAX_NUMBER under one lock and then appends to the
// symbol vector under another, so concurrent callers race and
// get_reverse_harmonization_vector can read a MAX_NUMBER ahead of the vector
// length and throw HfstFatalException. The C++ test suite never hits this
// because each C++ test is its own process; cargo runs every #[test] as a
// parallel thread in ONE process. Serializing the tests through this lock
// restores the one-at-a-time-per-process model without touching the library or
// weakening any assertion. into_inner() recovers from a poisoned lock so one
// failing test does not cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// Shared helper inlined from test/libhfst/auxiliary_functions.cc (verbose_print).
// get_bin is also defined there but is unused by this suite, so it is omitted.
fn verbose_print(msg: &str, ty: ImplementationType) {
    eprintln!("Testing:\t{msg} for type {ty:?}...");
}

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

fn temp_path(stem: &str) -> String {
    std::env::temp_dir()
        .join(stem)
        .to_str()
        .unwrap()
        .to_string()
}

// Local inverse of FromAnyTransducer: wrap a typed transducer into the runtime
// sum so the library's typed cross-backend conversion (AnyTransducer::into_typed,
// which preserves the facade metadata exactly as the old convert did) can be
// invoked from helpers generic over B.
trait IntoAny: AlgebraBackend {
    fn into_any(t: HfstTransducer<Self>) -> AnyTransducer;
}
impl IntoAny for StdVectorFst {
    fn into_any(t: HfstTransducer<Self>) -> AnyTransducer {
        AnyTransducer::Tropical(t)
    }
}

// --- The empty / epsilon / one-transition constructors, plus the destructor.
// These C++ blocks construct without asserting; the port verifies they do not
// panic.
fn smoke_constructors<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    verbose_print("The empty transducer", B::TYPE);
    let _empty = HfstTransducer::<B>::new();

    verbose_print("The epsilon transducer", B::TYPE);
    let _epsilon = HfstTransducer::<B>::new_symbol("@_EPSILON_SYMBOL_@")?;

    verbose_print("One-transition transducer", B::TYPE);
    let _foo = HfstTransducer::<B>::new_symbol("foo")?;
    let _foobar = HfstTransducer::<B>::new_symbol_pair("foo", "bar")?;

    // Destructor: C++ does 'new HfstTransducer("new", type); delete nu;'.
    verbose_print("Destructor", B::TYPE);
    let nu = Box::new(HfstTransducer::<B>::new_symbol("new")?);
    drop(nu);
    Ok(())
}

// --- The copy constructor.
fn copy_constructor<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    verbose_print("The copy constructor", B::TYPE);
    let foobar = HfstTransducer::<B>::new_symbol_pair("foo", "bar")?;
    let foobar_copy = HfstTransducer::new_copy(&foobar)?;
    assert!(foobar.compare_default(&foobar_copy)?);
    Ok(())
}

// --- Conversion from HfstBasicTransducer.
fn conversion_from_basic<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    verbose_print("Conversion from HfstBasicTransducer", B::TYPE);
    let foobar = HfstTransducer::<B>::new_symbol_pair("foo", "bar")?;

    let mut basic = HfstBasicTransducer::new();
    basic.add_state(1);
    let tr =
        HfstBasicTransition::new_symbols(1, "foo".into(), "bar".into(), 0.0, basic.coder_mut());
    basic.add_transition(0, &tr, true);
    basic.set_final_weight(1, &0.0);

    let foobar_basic = HfstTransducer::<B>::new_from_basic(&basic)?;
    assert!(foobar.compare_default(&foobar_basic)?);
    Ok(())
}

// --- Construction by tokenization.
fn construction_by_tokenization<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    verbose_print("Construction by tokenization", B::TYPE);
    let single = HfstTransducer::<B>::new_symbol("foo")?;
    let foobar = HfstTransducer::<B>::new_symbol_pair("foo", "bar")?;

    let mut tok = HfstTokenizer::new();
    tok.add_skip_symbol("baz");
    tok.add_multichar_symbol("foo");
    tok.add_multichar_symbol("bar");

    let foo_tok = HfstTransducer::<B>::new_tokenized("bazfoobaz", &tok)?;
    let foobar_tok = HfstTransducer::<B>::new_tokenized_pair("bazfoo", "barbaz", &tok)?;
    assert!(single.compare_default(&foo_tok)?);
    assert!(foobar.compare_default(&foobar_tok)?);
    Ok(())
}

// --- Construction from AT&T format.
fn construction_from_att<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    verbose_print("Construction from AT&T format", B::TYPE);
    let foobar = HfstTransducer::<B>::new_symbol_pair("foo", "bar")?;

    let path = fixture_path("foobar.att");
    // C++ uses the (FILE*, type, epsilon_symbol, linecount) constructor with
    // epsilon "@0@"; read_in_att_format_filename is the facade equivalent that
    // opens the file itself. warn_negs defaults to false.
    let mut foobar_att = HfstTransducer::<B>::read_in_att_format_filename(&path, "@0@", false)
        .expect("foobar.att fixture reads as a valid AT&T transducer");
    foobar_att.minimize()?;
    assert!(foobar.compare_default(&foobar_att)?);
    Ok(())
}

// --- Construction from HfstInputStream (also tests get_type, set_name, get_name).
fn construction_from_stream<B: AlgebraBackend + FromAnyTransducer>()
-> Result<(), hfst::error::Error> {
    verbose_print("Construction from HfstInputStream", B::TYPE);
    let mut foobar = HfstTransducer::<B>::new_symbol_pair("foo", "bar")?;

    let path = temp_path(&format!("hfst_test_constructors{:?}.hfst", B::TYPE));
    {
        let mut out = HfstOutputStream::new_filename(&path, foobar.get_type(), true)?;
        foobar.set_name("foobar");
        out.write(&mut foobar)?;
        out.close();
    }
    let mut instream = HfstInputStream::new_filename(&path)?;
    // The C++ 'HfstTransducer(instream)': the stream now reads the runtime sum;
    // this is a known-type read, so extract the typed transducer.
    let foobar_stream: HfstTransducer<B> = instream.read()?.into_typed()?;
    instream.close();
    let _ = std::fs::remove_file(&path);

    assert!(foobar.compare_default(&foobar_stream)?);
    assert_eq!(foobar_stream.get_name(), "foobar");
    assert_eq!(foobar_stream.get_type(), B::TYPE);
    Ok(())
}

// --- Operator= (the non-OL part of the C++ operator= block).
fn operator_assign<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    verbose_print("Operator=", B::TYPE);
    // In C++ foobar already carries name "foobar" from the stream block; set it
    // explicitly here so this group is self-contained.
    let mut foobar = HfstTransducer::<B>::new_symbol_pair("foo", "bar")?;
    foobar.set_name("foobar");

    let mut foobar2 = HfstTransducer::<B>::new_symbol("baz")?;
    assert_eq!(foobar.get_name(), "foobar");
    foobar2.operator_assign(&foobar)?;
    assert_eq!(foobar2.get_name(), "foobar");
    assert!(foobar.compare_default(&foobar2)?);
    Ok(())
}

// --- Reserving props in the copy constructor (C++ bug #3405831).
// Type-independent: always uses the tropical backend as the source, then
// converts to the HFST_OLW backend and checks the copy constructor preserves
// the name. The old 'convert(HFST_OLW_TYPE)' is the typed cross-backend
// conversion AnyTransducer::into_typed (which, like convert, preserves the
// facade metadata).
fn copy_constructor_preserves_name_after_olw_convert() -> Result<(), hfst::error::Error> {
    let t = HfstTransducer::<StdVectorFst>::new_symbol("a")?;
    let mut t: HfstTransducer<Transducer<WeightedTables>> =
        AnyTransducer::Tropical(t).into_typed()?;
    t.set_name("foo");
    let s = HfstTransducer::new_copy(&t)?;
    assert_eq!(s.get_name(), t.get_name());
    Ok(())
}

// --- The HFST_OL / HFST_OLW part of the C++ operator= block.
// Faithful port: constructs empty OL transducers and assigns converted copies
// of foobar2 into them, checking the name survives. The typed equivalent of
// 'convert' is AnyTransducer::into_typed, which preserves the facade metadata
// (the name) exactly as convert did. Under the monomorphic-backend interim
// invariant an in-memory conversion to EITHER OL type builds weighted-shaped
// tables (the OL/OLW distinction is payload data;
// Transducer<UnweightedTables> only arises from a disk load, its from_basic
// reports an error by design), so both the HFST_OL and the HFST_OLW arm of
// the C++ block land on Transducer<WeightedTables>. Both arms and both name
// assertions are kept.
fn operator_assign_ol<B: AlgebraBackend + IntoAny>() -> Result<(), hfst::error::Error> {
    // foobar2 corresponds to C++ foobar2 after 'foobar2 = foobar': the foo:bar
    // transducer named "foobar".
    let mut foobar2 = HfstTransducer::<B>::new_symbol_pair("foo", "bar")?;
    foobar2.set_name("foobar");

    let mut empty_ol = HfstTransducer::<Transducer<WeightedTables>>::new();
    let mut empty_olw = HfstTransducer::<Transducer<WeightedTables>>::new();

    let conv_ol: HfstTransducer<Transducer<WeightedTables>> =
        B::into_any(foobar2.clone()).into_typed()?;
    let conv_olw: HfstTransducer<Transducer<WeightedTables>> = B::into_any(foobar2).into_typed()?;
    empty_ol.operator_assign(&conv_ol)?;
    empty_olw.operator_assign(&conv_olw)?;
    assert_eq!(empty_ol.get_name(), "foobar");
    assert_eq!(empty_olw.get_name(), "foobar");
    Ok(())
}

// =====================================================================
// TROPICAL_OPENFST_TYPE (StdVectorFst)
// =====================================================================

#[test]
fn smoke_constructors_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    smoke_constructors::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn copy_constructor_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    copy_constructor::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn conversion_from_basic_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    conversion_from_basic::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn construction_by_tokenization_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    construction_by_tokenization::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn construction_from_att_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    construction_from_att::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn construction_from_stream_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    construction_from_stream::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn operator_assign_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    operator_assign::<StdVectorFst>()?;
    Ok(())
}

// =====================================================================
// HFST_OL / HFST_OLW usages (fixed types inside the C++ operator= block)
// =====================================================================

#[test]
fn copy_constructor_preserves_name_after_olw_convert_test() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    copy_constructor_preserves_name_after_olw_convert()?;
    Ok(())
}

#[test]
fn operator_assign_ol_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    operator_assign_ol::<StdVectorFst>()?;
    Ok(())
}
