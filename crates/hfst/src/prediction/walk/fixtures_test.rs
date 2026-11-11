//! Small generators compiled in the test with the library's own lexc and
//! regular expression compilers, and the grammars that go with them.

use hfst_openfst::StdVectorFst;

use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::lexc::LexcCompiler;
use crate::prediction::units::{UnitGrammar, UnitKey};
use crate::prediction::walk::{Candidate, Next, WalkLimits, Walker, Word};
use crate::transducer::{OlMember, Transducer, UnweightedTables, WeightedTables};
use crate::xre::XreCompiler;

/// Compiling writes the tropical backend's process-wide symbol table, so
/// tests that share a process take turns.
static COMPILING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn compiling() -> std::sync::MutexGuard<'static, ()> {
    COMPILING.lock().unwrap_or_else(|e| e.into_inner())
}

fn compiled_lexc(source: &str) -> HfstBasicTransducer {
    LexcCompiler::<StdVectorFst>::new()
        .compile(source)
        .expect("the test lexicon compiles")
        .to_basic()
        .expect("the lexicon is a transducer")
}

pub(in crate::prediction) fn lexc(source: &str) -> Transducer {
    let _turn = compiling();
    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(&compiled_lexc(source), true, "", None)
        .expect("the lexicon converts")
}

/// The lexicon as an unweighted optimized-lookup transducer, read back from
/// the bytes it writes, as a host loads one.
pub(in crate::prediction) fn unweighted_lexc(source: &str) -> Transducer<UnweightedTables> {
    let _turn = compiling();
    let written = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
        &compiled_lexc(source),
        false,
        "",
        None,
    )
    .expect("the lexicon converts");
    let mut bytes = Vec::new();
    written.write(&mut bytes);
    match OlMember::read_from(&mut std::io::Cursor::new(bytes), None)
        .expect("the written transducer reads back")
    {
        OlMember::Unweighted(read) => read,
        OlMember::Weighted(_) => panic!("an unweighted transducer reads back weighted"),
    }
}

pub(in crate::prediction) fn regex(source: &str) -> Transducer {
    let _turn = compiling();
    let compiled = XreCompiler::<StdVectorFst>::new()
        .compile(source)
        .expect("the test expression compiles");
    ConversionFunctions::hfst_transducer_to_hfst_ol(&compiled).expect("the expression converts")
}

/// Reads `a` three ways: writing `x` into a state with final weight 1.5,
/// writing `y` at 0.5 into one with final weight 0.25, and writing `x` at 1
/// into one with final weight 0.
pub(in crate::prediction) fn final_weights() -> Transducer {
    let _turn = compiling();
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for (target, output, weight, final_weight) in
        [(1, "x", 0.0, 1.5), (2, "y", 0.5, 0.25), (3, "x", 1.0, 0.0)]
    {
        let arc = HfstBasicTransition::new_symbols(
            target,
            "a".into(),
            output.into(),
            weight,
            net.coder_mut(),
        );
        net.add_transition(0, &arc, true);
        net.set_final_weight(target, &final_weight);
    }
    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(&net, true, "", None)
        .expect("the transducer converts")
}

pub(in crate::prediction) fn grammar(text: &str) -> UnitGrammar {
    UnitGrammar::from_toml(text.as_bytes()).expect("the test grammar reads")
}

/// North Sámi in small: nouns with a semantic tag the grammar hides, an
/// ending a longer ending extends, a question enclitic, a compound join
/// closed by `#`, a derivation, and endings of different weights, one with
/// a heavier second spelling.
pub(in crate::prediction) const NOUNS: &str = r#"
Multichar_Symbols
+N +V +Sg +Pl +Nom +Gen +PxSg1 +Qst +Cmp +Der/NomAg +Inf +Sem/Obj

LEXICON Root
Nouns ;
Verbs ;

LEXICON Nouns
girji:girj N ;
guolli:guol N ;
gáhkku:gáhkk N ;

LEXICON N
+N:0 NInfl ;
+N+Sem/Obj:0 NInfl ;
+N+Cmp:0 Join ;

LEXICON Join
%#:0 Nouns ;

LEXICON NInfl
+Sg+Nom:i End ;
+Sg+Nom:ii End "weight: 2" ;
+Sg+Nom+PxSg1:an End "weight: 1" ;
+Sg+Gen:a End "weight: 3" ;
+Pl+Nom:at End "weight: 4" ;

LEXICON End
# ;
+Qst:go # ;

LEXICON Verbs
borra:borr V ;

LEXICON V
+V+Inf:at # ;
+V+Der/NomAg:i N ;
"#;

pub(in crate::prediction) const NOUNS_GRAMMAR: &str = r##"
tags = ["#"]
boundary = ["+Der/*", "#"]
enclitic = ["+Qst"]
hidden = ["+Sem/*"]

[labels]
"+V+Der/NomAg" = "-eaddji"
"##;

/// Plains Cree in small: the generator writes ni- or ki- before the stem
/// and sets a person flag that the ending must agree with, and a preverb
/// can come first.
pub(in crate::prediction) const PERSONS: &str = r#"
Multichar_Symbols
@U.person.NI@ @U.person.KI@ @U.person.NONE@ PV/kii+ +V +AI +Ind +1Sg +2Sg +3Sg

LEXICON Root
@U.person.NI@ First ;
@U.person.KI@ Second ;
@U.person.NONE@ Preverbs ;

LEXICON First
0:ni Preverbs ;

LEXICON Second
0:ki Preverbs ;

LEXICON Preverbs
PV/kii+:kii- Stems ;
Stems ;

LEXICON Stems
nipaa:nipaa VAI ;
pimi:pimi VAI ;

LEXICON VAI
+V+AI+Ind:0 Agree ;

LEXICON Agree
@U.person.NI@ I ;
@U.person.KI@ You ;
@U.person.NONE@ They ;

LEXICON I
+1Sg:n # ;

LEXICON You
+2Sg:n # ;

LEXICON They
+3Sg:w # ;
"#;

pub(in crate::prediction) const PERSONS_GRAMMAR: &str = r#"
prefix = ["PV/*"]

[[filter]]
name = "ni-"
label = "ni-"
flags = ["@U.person.NI@"]

[[filter]]
name = "ki-"
label = "ki-"
flags = ["@U.person.KI@"]

[labels]
"PV/kii+" = "kii-"
"#;

/// A tag that repeats without end, and a stem whose path writes letters
/// without end before a tag.
pub(in crate::prediction) const CYCLES: &str = r#"
Multichar_Symbols
+N +Rep +Sg

LEXICON Root
aa:aa Tags ;
bb:bb Echo ;

LEXICON Tags
+Rep:0 Tags ;
+N+Sg:0 # ;

LEXICON Echo
0:x Echo ;
+N+Sg:0 # ;
"#;

pub(in crate::prediction) fn walker<'a>(
    generator: &'a Transducer,
    grammar: &'a UnitGrammar,
) -> Walker<'a, WeightedTables> {
    Walker::new(generator, grammar, WalkLimits::default()).expect("the grammar fits the generator")
}

pub(in crate::prediction) fn keys(next: &Next) -> Vec<&str> {
    next.candidates
        .iter()
        .map(|candidate| candidate.key.as_str())
        .collect()
}

pub(in crate::prediction) fn candidate<'n>(next: &'n Next, key: &str) -> &'n Candidate {
    next.candidates
        .iter()
        .find(|candidate| candidate.key == UnitKey::from(key))
        .unwrap_or_else(|| panic!("{key} is offered among {:?}", keys(next)))
}

pub(in crate::prediction) fn surfaces(
    walker: &Walker<'_, WeightedTables>,
    word: &Word,
) -> Vec<String> {
    walker
        .forms(word)
        .into_iter()
        .map(|form| form.surface)
        .collect()
}

/// The word reached by typing `stem` and picking it, then picking each of
/// `units` in turn.
pub(in crate::prediction) fn word_after(
    walker: &Walker<'_, WeightedTables>,
    stem: &str,
    units: &[&str],
) -> Word {
    let start = walker.start();
    let entered = walker
        .entry(&start, stem, None)
        .expect("a stem can start the word");
    let mut word = start.pick(candidate(&entered, stem));
    for unit in units {
        let next = walker.next(&word);
        word = word.pick(candidate(&next, unit));
    }
    word
}

/// Greenlandic in small: a derivation's Der tag may be left out, so a
/// morpheme name can go straight on to an ending or to the next morpheme
/// name, without end. `+LI` begins a derivation after a stem and is an
/// enclitic after a complete word, and a proper noun's tag comes before
/// its derivation tag.
pub(in crate::prediction) const DERIVATIONS: &str = r#"
Multichar_Symbols
+V +N +Abs +Sg +Ind +3Sg +NIQ +KIP +LI +Der/vn +Der/nv +Der/vv +Gram/IV
+Prop +U

LEXICON Root
oqar:oqar Verb ;
Akoq:Akoq Proper ;

LEXICON Proper
+Prop+Abs:0 End ;
+Prop+U:u Verb ;
+Prop+U+Der/nv:u Verb ;

LEXICON Verb
+V+Ind+3Sg:poq End ;
Derive ;
+Gram/IV:0 Derive ;

LEXICON Derive
+NIQ:niq Noun ;
+NIQ+Der/vn:niq Noun ;
+LI+Der/vv:li Verb ;
+KIP:kip Verb ;
+KIP+Der/nv:kip Verb ;

LEXICON Noun
+N+Abs+Sg:0 End ;
Derive ;

LEXICON End
# ;
+LI:lu # ;
"#;

pub(in crate::prediction) const DERIVATIONS_GRAMMAR: &str = r#"
boundary = ["+Der/*"]
derivation = ["+NIQ", "+KIP", "+LI", "+U"]
enclitic = ["+LI"]
hidden = ["+Gram/*"]
"#;

/// The same grammar without its derivation tags.
pub(in crate::prediction) const NO_DERIVATIONS_GRAMMAR: &str = r#"
boundary = ["+Der/*"]
enclitic = ["+LI"]
hidden = ["+Gram/*"]
"#;
