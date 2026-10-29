// DHFST acceptor lookup against the optimized-lookup transducer the acceptor
// is written from. Each fixture is AT&T text, compiled to weighted optimized
// lookup and written as an acceptor the way `hfst fst2fst -f dhfst
// --dhfst-type acceptor` writes one. Where every symbol is one grapheme
// cluster, the acceptor's least weight must be the transducer's least lookup
// weight for every word; where it is not, lookup must answer as divvunspell's
// speller does, which the tests spell out.
//
// The tropical transition-data symbol coding behind `HfstBasicTransducer`
// lives in process-global statics, so construction is serialized, as in the
// sibling OL test files.

use hfst::dhfst_acceptor::AcceptorReader;
use hfst::dhfst_acceptor_writer::{AcceptorOptions, AcceptorSource, write};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_transducer::HfstTransducer;
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

struct Fixture {
    olw: HfstTransducer<Transducer<WeightedTables>>,
    dhfst: Vec<u8>,
}

impl Fixture {
    /// The fixture of AT&T `lines`, each a tab-separated arc or final state.
    fn new(lines: &[&str]) -> Fixture {
        let att: String = lines.iter().map(|line| format!("{line}\n")).collect();
        let _guard = serialized();
        let mut read = 0u32;
        let net =
            HfstBasicTransducer::read_in_att_format(&mut att.as_bytes(), "@0@", &mut read, false)
                .expect("the AT&T text parses");
        let olw = HfstTransducer::<StdVectorFst>::new_from_basic(&net)
            .expect("the fixture is a transducer")
            .to_ol(true, "")
            .expect("the fixture converts to weighted optimized lookup");
        let source = AcceptorSource::new(&olw).expect("the fixture opens as an acceptor source");
        let dhfst = write(
            &source,
            &AcceptorOptions {
                threads: 1,
                source_name: "fixture".into(),
            },
        )
        .expect("the fixture writes as a DHFST acceptor")
        .bytes;
        Fixture { olw, dhfst }
    }

    fn reader(&self) -> AcceptorReader<'_> {
        AcceptorReader::parse(&self.dhfst).expect("the written acceptor loads")
    }

    /// The least weight optimized lookup finds for `word`, flags obeyed.
    fn ol_least(&self, word: &str) -> Option<f32> {
        self.olw
            .lookup_state()
            .lookup_fd(word, -1, 0.0)
            .iter()
            .map(|path| path.first)
            .min_by(f32::total_cmp)
    }
}

/// Every string of at most `max` characters drawn from `letters`.
fn strings(letters: &[char], max: usize) -> Vec<String> {
    let mut all = vec![String::new()];
    let mut last = vec![String::new()];
    for _ in 0..max {
        last = last
            .iter()
            .flat_map(|s| letters.iter().map(move |c| format!("{s}{c}")))
            .collect();
        all.extend(last.iter().cloned());
    }
    all
}

/// Weights, epsilon arcs, an epsilon cycle, a flag cycle, two arcs on one
/// symbol, and each flag operator deciding which path is the lightest.
const FLAGS: &[&str] = &[
    // P and R: talo takes CASE NOM, talon CASE GEN.
    "0\t1\t@P.CASE.NOM@\t@P.CASE.NOM@\t0",
    "0\t2\t@P.CASE.GEN@\t@P.CASE.GEN@\t0.5",
    "1\t3\tt\tt\t0.25",
    "2\t3\tt\tt\t0",
    "3\t19\t@0@\t@0@\t0",
    "19\t3\t@0@\t@0@\t0",
    "3\t4\ta\ta\t0",
    "4\t18\t@P.CASE.NOM@\t@P.CASE.NOM@\t0.0625",
    "18\t4\t@0@\t@0@\t0",
    "4\t5\tl\tl\t0.125",
    "5\t6\to\to\t0",
    "6\t7\t@R.CASE.NOM@\t@R.CASE.NOM@\t0",
    "6\t8\t@R.CASE.GEN@\t@R.CASE.GEN@\t0",
    "7\t0.5",
    "8\t9\tn\tn\t1",
    "9\t0",
    // N, then U, D and R with and without a value.
    "0\t10\t@N.NUM.SG@\t@N.NUM.SG@\t0",
    "10\t11\tk\tk\t0",
    "11\t13\t@U.NUM.SG@\t@U.NUM.SG@\t0",
    "13\t15\ti\ti\t0",
    "11\t14\t@U.NUM.PL@\t@U.NUM.PL@\t0.25",
    "14\t16\ti\ti\t0",
    "16\t17\t@R.NUM.PL@\t@R.NUM.PL@\t0",
    "15\t0",
    "17\t0.25",
    "11\t21\t@D.NUM.SG@\t@D.NUM.SG@\t0",
    "21\t22\te\te\t0.25",
    "11\t23\t@D.NUM@\t@D.NUM@\t0",
    "23\t22\te\te\t0",
    "22\t0",
    "11\t24\t@R.NUM@\t@R.NUM@\t0",
    "24\t25\to\to\t0.125",
    "11\t26\t@R.NUM.SG@\t@R.NUM.SG@\t0",
    "26\t25\to\to\t0",
    "25\t0",
    // C: m clears X, which D without a value then needs unset.
    "0\t30\t@P.X.A@\t@P.X.A@\t0",
    "30\t31\t@C.X@\t@C.X@\t0",
    "31\t32\tm\tm\t0",
    "32\t33\t@D.X@\t@D.X@\t0.25",
    "33\t0",
    "32\t34\t@R.X@\t@R.X@\t0",
    "34\t0",
    // Two arcs on c, and the empty word through an epsilon arc.
    "0\t40\tc\tc\t0.5",
    "0\t41\tc\tc\t1",
    "40\t42\ta\ta\t0",
    "41\t42\ta\ta\t0",
    "42\t0.125",
    "0\t50\t@0@\t@0@\t2",
    "50\t1",
];

// [spec:hfst:req:javascript.dhfst+1/test]
#[test]
fn lookup_matches_optimized_lookup_through_flags() {
    let fixture = Fixture::new(FLAGS);
    let reader = fixture.reader();
    for (word, weight) in [
        ("talo", Some(0.875)),
        ("talon", Some(1.625)),
        ("ki", Some(0.5)),
        ("ke", Some(0.25)),
        ("ko", Some(0.125)),
        ("m", Some(0.25)),
        ("ca", Some(0.625)),
        ("", Some(3.0)),
        ("tal", None),
        ("k", None),
        ("z", None),
    ] {
        assert_eq!(reader.lookup(word), weight, "{word:?}");
    }
    let mut accepted = 0;
    for word in strings(&['t', 'a', 'l', 'o', 'n', 'k', 'i', 'e', 'm', 'c', 'z'], 4)
        .into_iter()
        .chain(["talon", "talos", "talonn", "ttalo"].map(String::from))
    {
        let weight = reader.lookup(&word);
        assert_eq!(
            weight.map(f32::to_bits),
            fixture.ol_least(&word).map(f32::to_bits),
            "{word:?}"
        );
        accepted += usize::from(weight.is_some());
    }
    assert_eq!(accepted, 8, "the words the fixture accepts");
}

// [spec:hfst:req:javascript.dhfst+1/test]
#[test]
fn free_arc_cycles_and_long_words_end() {
    let fixture = Fixture::new(&[
        "0\t0\ta\ta\t0.5",
        "0\t1\t@0@\t@0@\t0",
        "1\t0\t@0@\t@0@\t0",
        "1\t2\t@P.F.A@\t@P.F.A@\t0",
        "2\t1\t@P.F.B@\t@P.F.B@\t0",
        "2\t0\t@0@\t@0@\t0",
        "0\t0",
    ]);
    let reader = fixture.reader();
    // Optimized lookup recurses once per arc of a path, and on these cycles
    // it recurses deep enough to overflow a test thread's stack, so the
    // weights are given rather than looked up.
    for word in strings(&['a', 'b'], 6) {
        let weight = (!word.contains('b')).then_some(0.5 * word.len() as f32);
        assert_eq!(reader.lookup(&word), weight, "{word:?}");
    }
    let long = "a".repeat(20_000);
    assert_eq!(reader.lookup(&long), Some(10_000.0));
    assert_eq!(reader.lookup(&format!("{long}b")), None);
}

// [spec:hfst:req:javascript.dhfst+1/test]
#[test]
fn multichar_symbols_are_not_read_from_graphemes() {
    let fixture = Fixture::new(&[
        "0\t1\tij\tij\t0.5",
        "1\t2\ta\ta\t0",
        "2\t0",
        "0\t3\ti\ti\t1",
        "3\t4\tj\tj\t1",
        "4\t5\ta\ta\t0",
        "5\t0",
        "0\t6\txy\txy\t0",
        "6\t0",
    ]);
    let reader = fixture.reader();
    assert_eq!(fixture.ol_least("ija"), Some(0.5), "lookup reads ij whole");
    assert_eq!(reader.lookup("ija"), Some(2.0), "the speller reads i, j, a");
    assert_eq!(fixture.ol_least("xy"), Some(0.0), "lookup reads xy whole");
    assert_eq!(reader.lookup("xy"), None, "no grapheme spells xy");
}

// [spec:hfst:req:javascript.dhfst+1/test]
#[test]
fn a_symbol_matches_only_its_own_normalisation_form() {
    let fixture = Fixture::new(&[
        "0\t1\t\u{e9}\t\u{e9}\t0.25",
        "1\t0",
        "0\t2\to\u{308}\to\u{308}\t0.5",
        "2\t0",
    ]);
    let reader = fixture.reader();
    assert_eq!(reader.lookup("\u{e9}"), Some(0.25), "é as stored, composed");
    assert_eq!(
        reader.lookup("o\u{308}"),
        Some(0.5),
        "ö as stored, decomposed"
    );
    assert_eq!(
        fixture.ol_least("e\u{301}"),
        Some(0.25),
        "lookup matches é decomposed"
    );
    assert_eq!(reader.lookup("e\u{301}"), None, "the speller does not");
    assert_eq!(
        fixture.ol_least("\u{f6}"),
        Some(0.5),
        "lookup matches ö composed"
    );
    assert_eq!(reader.lookup("\u{f6}"), None, "the speller does not");
}

// [spec:hfst:req:javascript.dhfst+1/test]
#[test]
fn unknown_graphemes_read_as_the_unknown_symbol() {
    let fixture = Fixture::new(&[
        "0\t1\ta\ta\t0",
        "1\t2\t@_UNKNOWN_SYMBOL_@\t@_UNKNOWN_SYMBOL_@\t0.5",
        "2\t0",
    ]);
    let reader = fixture.reader();
    for word in ["aq", "a\u{1F600}"] {
        assert_eq!(reader.lookup(word), Some(0.5), "{word:?}");
        assert_eq!(fixture.ol_least(word), Some(0.5), "{word:?}");
    }
    assert_eq!(
        reader.lookup("ae\u{301}"),
        Some(0.5),
        "one cluster is one symbol"
    );
    assert_eq!(
        reader.lookup("aa"),
        None,
        "a is in the alphabet, so not unknown"
    );
    assert_eq!(reader.lookup("a"), None);
    let without = Fixture::new(&["0\t1\ta\ta\t0", "1\t0"]);
    assert_eq!(
        without.reader().lookup("q"),
        None,
        "no unknown symbol, no path"
    );
}

// [spec:hfst:req:javascript.dhfst+1/test]
#[test]
fn owned_reader_agrees_and_corrupt_files_fail() {
    let fixture = Fixture::new(FLAGS);
    let borrowed = fixture.reader();
    let owned = AcceptorReader::parse_owned(fixture.dhfst.clone()).expect("the acceptor loads");
    assert_eq!(owned.version(), 1);
    assert_eq!(owned.symbol_names(), borrowed.symbol_names());
    for word in ["talo", "talon", "ki", "", "zz"] {
        assert_eq!(owned.lookup(word), borrowed.lookup(word), "{word:?}");
    }
    let mut error_model = fixture.dhfst.clone();
    error_model[5] = 1;
    let mut future = fixture.dhfst.clone();
    future[6] = 2;
    for (bytes, what) in [
        (
            fixture.dhfst[..fixture.dhfst.len() / 2].to_vec(),
            "truncated",
        ),
        (
            fixture.dhfst[..30].to_vec(),
            "truncated in the section table",
        ),
        (error_model, "an error model"),
        (future, "version 2"),
        (b"DHFST".to_vec(), "only the magic"),
    ] {
        assert!(AcceptorReader::parse(&bytes).is_err(), "{what} loads");
        assert!(
            AcceptorReader::parse_owned(bytes).is_err(),
            "{what} loads owned"
        );
    }
}
