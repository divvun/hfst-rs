//! The options of each operation, as the tool it is named after takes them:
//! the tool's flags in camelCase, checked as the tool checks them, each left
//! out at the tool's default.

use hfst_embed::Code;
use hfst_embed::compile::{
    AffixGuessifyOptions, BinaryOptions, ComposeIntersectOptions, ComposeOptions, ConvertOptions,
    Count, DeterminizeOptions, DisjunctOptions, EditMetadataOptions, EliminateFlagsOptions,
    ExpandEquivalencesOptions, GuessDirection, GuessifyOptions, InsertFreelyOptions,
    KillPathsOptions, Level, MinimizeOptions, NameOptions, ProjectOptions, PruneAlphabetOptions,
    PushDirection, PushOptions, RepeatOptions, ReweightOptions, Tape,
};
use hfst_wasm_shared::GlueError;

use crate::options::Options;

const PUSH: [(&str, PushDirection); 2] = [
    ("initial", PushDirection::Initial),
    ("final", PushDirection::Final),
];

/// The functions `hfst reweight --function` applies to a weight.
const FUNCTIONS: [&str; 15] = [
    "cos", "sin", "tan", "acos", "asin", "atan", "cosh", "sinh", "tanh", "exp", "log", "log10",
    "sqrt", "floor", "ceil",
];

/// A bound of `repeat`: a whole number of times, or `Infinity`. The library
/// counts repetitions in 32 bits.
fn repetitions(o: &Options<'_>, name: &str, default: Count) -> Result<Count, GlueError> {
    let expected = format!("a whole number from 0 to {}, or Infinity", u32::MAX);
    let bound = o.number(name, &expected, |n| {
        n == f64::INFINITY || (n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&n))
    })?;
    Ok(match bound {
        None => default,
        Some(n) if n == f64::INFINITY => Count::Infinite,
        Some(n) => Count::Finite(n as u64),
    })
}

pub fn determinize(o: &Options<'_>) -> Result<DeterminizeOptions, GlueError> {
    o.only(&["encodeWeights"])?;
    Ok(DeterminizeOptions {
        encode_weights: o.flag("encodeWeights", false)?,
    })
}

pub fn minimize(o: &Options<'_>) -> Result<MinimizeOptions, GlueError> {
    o.only(&["encodeWeights"])?;
    Ok(MinimizeOptions {
        encode_weights: o.flag("encodeWeights", false)?,
    })
}

pub fn prune_alphabet(o: &Options<'_>) -> Result<PruneAlphabetOptions, GlueError> {
    o.only(&["force"])?;
    Ok(PruneAlphabetOptions {
        force: o.flag("force", false)?,
    })
}

pub fn project(o: &Options<'_>) -> Result<ProjectOptions, GlueError> {
    o.only(&["project"])?;
    let tapes = [("input", Tape::Input), ("output", Tape::Output)];
    Ok(ProjectOptions {
        project: o.choice("project", &tapes)?.unwrap_or(Tape::Output),
    })
}

/// `pushWeights`' and `pushLabels`'.
pub fn push(o: &Options<'_>) -> Result<PushOptions, GlueError> {
    o.only(&["push"])?;
    Ok(PushOptions {
        push: o.choice("push", &PUSH)?.unwrap_or(PushDirection::Final),
    })
}

pub fn repeat(o: &Options<'_>) -> Result<RepeatOptions, GlueError> {
    o.only(&["from", "to"])?;
    Ok(RepeatOptions {
        from: repetitions(o, "from", Count::Finite(0))?,
        to: repetitions(o, "to", Count::Infinite)?,
    })
}

pub fn insert_freely(o: &Options<'_>) -> Result<InsertFreelyOptions, GlueError> {
    o.only(&["symbolPair", "harmonise"])?;
    let symbol_pair = o.text("symbolPair")?.ok_or_else(|| {
        GlueError::new(
            Code::InvalidArgument,
            "insertFreely takes the pair to insert as its option symbolPair",
        )
    })?;
    Ok(InsertFreelyOptions {
        symbol_pair,
        harmonise: o.flag("harmonise", false)?,
    })
}

pub fn eliminate_flags(o: &Options<'_>) -> Result<EliminateFlagsOptions, GlueError> {
    o.only(&["flag"])?;
    Ok(EliminateFlagsOptions {
        flag: o.text("flag")?,
    })
}

pub fn affix_guessify(o: &Options<'_>) -> Result<AffixGuessifyOptions, GlueError> {
    o.only(&["direction", "weight"])?;
    let directions = [
        ("prefix", GuessDirection::GuessPrefix),
        ("suffix", GuessDirection::GuessSuffix),
    ];
    Ok(AffixGuessifyOptions {
        direction: o
            .choice("direction", &directions)?
            .unwrap_or(GuessDirection::GuessSuffix),
        weight: o.weight("weight")?.unwrap_or(1.0),
    })
}

pub fn expand_equivalences(o: &Options<'_>) -> Result<ExpandEquivalencesOptions, GlueError> {
    o.only(&["from", "to", "tsv", "level"])?;
    let levels = [
        ("first", Level::First),
        ("second", Level::Second),
        ("both", Level::Both),
    ];
    Ok(ExpandEquivalencesOptions {
        from: o.text("from")?,
        to: o.text("to")?,
        tsv: o.text("tsv")?,
        level: o.choice("level", &levels)?.unwrap_or(Level::First),
    })
}

pub fn kill_paths(o: &Options<'_>) -> Result<KillPathsOptions, GlueError> {
    o.only(&["symbol", "tsv"])?;
    Ok(KillPathsOptions {
        symbol: o.text("symbol")?,
        tsv: o.text("tsv")?,
    })
}

pub fn reweight(o: &Options<'_>) -> Result<ReweightOptions, GlueError> {
    o.only(&[
        "addition",
        "multiplier",
        "function",
        "lowerBound",
        "upperBound",
        "inputSymbol",
        "outputSymbol",
        "symbol",
        "endStatesOnly",
        "arcsOnly",
        "tsv",
    ])?;
    let functions: Vec<(&str, &str)> = FUNCTIONS.iter().map(|f| (*f, *f)).collect();
    let defaults = ReweightOptions::default();
    Ok(ReweightOptions {
        addition: o.weight("addition")?.unwrap_or(defaults.addition),
        multiplier: o.weight("multiplier")?.unwrap_or(defaults.multiplier),
        function: o.choice("function", &functions)?.map(str::to_owned),
        lower_bound: o.weight("lowerBound")?.unwrap_or(defaults.lower_bound),
        upper_bound: o.weight("upperBound")?.unwrap_or(defaults.upper_bound),
        input_symbol: o.text("inputSymbol")?,
        output_symbol: o.text("outputSymbol")?,
        symbol: o.text("symbol")?,
        end_states_only: o.flag("endStatesOnly", false)?,
        arcs_only: o.flag("arcsOnly", false)?,
        tsv: o.text("tsv")?,
    })
}

pub fn set_name(name: String, o: &Options<'_>) -> Result<NameOptions, GlueError> {
    o.only(&["truncateLength"])?;
    Ok(NameOptions {
        name,
        truncate_length: o.usize("truncateLength", 0)?.unwrap_or(0),
    })
}

pub fn edit_metadata(o: &Options<'_>) -> Result<EditMetadataOptions, GlueError> {
    o.only(&["add", "truncateLength"])?;
    Ok(EditMetadataOptions {
        add: o.record("add")?.unwrap_or_default(),
        truncate_length: o.usize("truncateLength", 0)?.unwrap_or(0),
    })
}

pub fn guessify(o: &Options<'_>) -> Result<GuessifyOptions, GlueError> {
    o.only(&["defaultPenalty", "doNotCompileGenerator"])?;
    Ok(GuessifyOptions {
        default_penalty: o.weight("defaultPenalty")?.unwrap_or(1.0),
        do_not_compile_generator: o.flag("doNotCompileGenerator", false)?,
    })
}

/// What `substitute` reads besides the transducer it may put in: the labels
/// or the relabel file, and the name of the file the tool would read either
/// from.
pub struct Substitution {
    pub from_label: Option<String>,
    pub to_label: Option<String>,
    pub from_file: Option<String>,
    pub file_name: String,
    pub in_order: bool,
    pub do_not_convert: bool,
    pub compose: bool,
}

pub fn substitute(o: &Options<'_>) -> Result<Substitution, GlueError> {
    o.only(&[
        "fromLabel",
        "toLabel",
        "fromFile",
        "fileName",
        "inOrder",
        "doNotConvert",
        "compose",
    ])?;
    Ok(Substitution {
        from_label: o.text("fromLabel")?,
        to_label: o.text("toLabel")?,
        from_file: o.text("fromFile")?,
        file_name: o.text("fileName")?.unwrap_or_default(),
        in_order: o.flag("inOrder", false)?,
        do_not_convert: o.flag("doNotConvert", false)?,
        compose: o.flag("compose", false)?,
    })
}

pub fn compose(o: &Options<'_>) -> Result<ComposeOptions, GlueError> {
    o.only(&[
        "doNotHarmonize",
        "harmonizeFlags",
        "xeroxComposition",
        "flagIsEpsilon",
        "doNotConvert",
    ])?;
    Ok(ComposeOptions {
        do_not_harmonize: o.flag("doNotHarmonize", false)?,
        harmonize_flags: o.flag("harmonizeFlags", false)?,
        xerox_composition: o.flag("xeroxComposition", false)?,
        flag_is_epsilon: o.flag("flagIsEpsilon", false)?,
        do_not_convert: o.flag("doNotConvert", false)?,
    })
}

/// `concatenate`'s, `conjunct`'s and `subtract`'s.
pub fn binary(o: &Options<'_>) -> Result<BinaryOptions, GlueError> {
    o.only(&["doNotHarmonize", "harmonizeFlags", "doNotConvert"])?;
    Ok(BinaryOptions {
        do_not_harmonize: o.flag("doNotHarmonize", false)?,
        harmonize_flags: o.flag("harmonizeFlags", false)?,
        do_not_convert: o.flag("doNotConvert", false)?,
    })
}

pub fn disjunct(o: &Options<'_>) -> Result<DisjunctOptions, GlueError> {
    o.only(&["doNotHarmonize", "doNotConvert"])?;
    Ok(DisjunctOptions {
        do_not_harmonize: o.flag("doNotHarmonize", false)?,
        do_not_convert: o.flag("doNotConvert", false)?,
    })
}

/// `priorityDisjunct`'s and `shuffle`'s.
pub fn conversion(o: &Options<'_>) -> Result<ConvertOptions, GlueError> {
    o.only(&["doNotConvert"])?;
    Ok(ConvertOptions {
        do_not_convert: o.flag("doNotConvert", false)?,
    })
}

pub fn compose_intersect(o: &Options<'_>) -> Result<ComposeIntersectOptions, GlueError> {
    o.only(&[
        "invert",
        "fast",
        "encodeWeights",
        "harmonize",
        "lexiconName",
        "rulesName",
        "doNotConvert",
    ])?;
    Ok(ComposeIntersectOptions {
        invert: o.flag("invert", false)?,
        fast: o.flag("fast", false)?,
        encode_weights: o.flag("encodeWeights", false)?,
        harmonize: o.flag("harmonize", false)?,
        lexicon_name: o.text("lexiconName")?.unwrap_or_default(),
        rules_name: o.text("rulesName")?.unwrap_or_default(),
        do_not_convert: o.flag("doNotConvert", false)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{Value, given};

    fn none() -> Options<'static> {
        given("operation", &[])
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn options_left_out_are_the_tools_defaults() {
        assert!(!determinize(&none()).expect("none").encode_weights);
        assert!(matches!(
            project(&none()).expect("none").project,
            Tape::Output
        ));
        assert!(matches!(
            push(&none()).expect("none").push,
            PushDirection::Final
        ));
        let repeat = repeat(&none()).expect("none");
        assert_eq!(
            (repeat.from, repeat.to),
            (Count::Finite(0), Count::Infinite)
        );
        let reweight = reweight(&none()).expect("none");
        let defaults = ReweightOptions::default();
        assert_eq!(reweight.upper_bound, defaults.upper_bound);
        assert_eq!(reweight.multiplier, 1.0);
        let guess = affix_guessify(&none()).expect("none");
        assert!(matches!(guess.direction, GuessDirection::GuessSuffix));
        assert_eq!(guess.weight, 1.0);
        assert_eq!(guessify(&none()).expect("none").default_penalty, 1.0);
        assert!(matches!(
            expand_equivalences(&none()).expect("none").level,
            Level::First
        ));
        assert_eq!(
            set_name("n".to_owned(), &none())
                .expect("none")
                .truncate_length,
            0
        );
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn options_are_the_tools_flags_in_camel_case() {
        let o = given(
            "repeat",
            &[("from", 1.0.into()), ("to", f64::INFINITY.into())],
        );
        let r = repeat(&o).expect("bounds");
        assert_eq!((r.from, r.to), (Count::Finite(1), Count::Infinite));
        let o = given(
            "reweight",
            &[
                ("addition", 1.0.into()),
                ("function", "sqrt".into()),
                ("endStatesOnly", true.into()),
                ("inputSymbol", "a".into()),
            ],
        );
        let r = reweight(&o).expect("reweight");
        assert_eq!(r.addition, 1.0);
        assert_eq!(r.function.as_deref(), Some("sqrt"));
        assert!(r.end_states_only);
        assert_eq!(r.input_symbol.as_deref(), Some("a"));
        let o = given(
            "editMetadata",
            &[
                ("add", Value::Record(vec![("foo".to_owned(), "bar".into())])),
                ("truncateLength", 2.0.into()),
            ],
        );
        let e = edit_metadata(&o).expect("metadata");
        assert_eq!(e.add, vec![("foo".to_owned(), "bar".to_owned())]);
        assert_eq!(e.truncate_length, 2);
        let o = given("compose", &[("xeroxComposition", true.into())]);
        assert!(compose(&o).expect("compose").xerox_composition);
        let o = given(
            "substitute",
            &[("fromLabel", "a".into()), ("inOrder", true.into())],
        );
        let s = substitute(&o).expect("substitute");
        assert_eq!(s.from_label.as_deref(), Some("a"));
        assert!(s.in_order && s.file_name.is_empty());
        let o = given("composeIntersect", &[("lexiconName", "l.lexc".into())]);
        assert_eq!(compose_intersect(&o).expect("names").lexicon_name, "l.lexc");
        let o = given("project", &[("project", "input".into())]);
        assert!(matches!(project(&o).expect("input").project, Tape::Input));
    }

    #[test]
    fn values_the_tools_turn_down_are_refused() {
        for (o, what) in [
            (
                given("repeat", &[("from", 1.5.into())]),
                "a fraction of a time",
            ),
            (
                given("repeat", &[("to", (-1.0).into())]),
                "a negative count",
            ),
            (
                given("reweight", &[("function", "cube".into())]),
                "no such function",
            ),
            (
                given("push", &[("push", "middle".into())]),
                "no such direction",
            ),
        ] {
            let refused = match what {
                "no such direction" => push(&o).err(),
                "no such function" => reweight(&o).err(),
                _ => repeat(&o).err(),
            };
            assert_eq!(
                refused.map(|e| e.code()),
                Some("invalid-argument"),
                "{what}"
            );
        }
        let missing = insert_freely(&none()).expect_err("no pair");
        assert!(
            missing.message().contains("symbolPair"),
            "{}",
            missing.message()
        );
        let unknown = binary(&given("concatenate", &[("doNotHarmonise", true.into())]))
            .expect_err("misspelt");
        assert!(
            unknown.message().contains("doNotHarmonise"),
            "{}",
            unknown.message()
        );
        assert!(disjunct(&given("disjunct", &[("harmonizeFlags", true.into())])).is_err());
        assert!(
            conversion(&given("shuffle", &[("doNotConvert", true.into())]))
                .expect("ok")
                .do_not_convert
        );
    }
}
