//! The options of each query, as the tool it is named after takes them: the
//! tool's flags in camelCase, checked as the tool checks them, each left out
//! at the tool's default. Times are in milliseconds, as the package's other
//! time limits are.

use hfst_embed::compile::{
    Cascade, CompareOptions, ExtractPathsOptions, FlookupOptions, GrepOptions, GuessOptions,
    LookupInput, LookupOptions, OptimizedLookupOptions, PairTestOptions, PmatchOptions,
};
use hfst_wasm_shared::GlueError;

use crate::options::Options;

const INPUT_FORMATS: [(&str, LookupInput); 3] = [
    ("text", LookupInput::Text),
    ("spaced", LookupInput::Spaced),
    ("apertium", LookupInput::Apertium),
];

const CASCADES: [(&str, Cascade); 3] = [
    ("union", Cascade::Union),
    ("priority-union", Cascade::PriorityUnion),
    ("composition", Cascade::Composition),
];

/// Whether `summarize` measures what `--verbose` measures.
pub fn summarize(o: &Options<'_>) -> Result<bool, GlueError> {
    o.only(&["verbose"])?;
    o.flag("verbose", false)
}

pub fn compare(o: &Options<'_>) -> Result<CompareOptions, GlueError> {
    o.only(&["doNotHarmonize", "eliminateFlags"])?;
    Ok(CompareOptions {
        do_not_harmonize: o.flag("doNotHarmonize", false)?,
        eliminate_flags: o.flag("eliminateFlags", false)?,
    })
}

pub fn extract_paths(o: &Options<'_>) -> Result<ExtractPathsOptions, GlueError> {
    o.only(&[
        "maxStrings",
        "nbest",
        "random",
        "cycles",
        "beam",
        "obeyFlags",
        "printFlags",
        "maxInLength",
        "maxOutLength",
        "inPrefix",
        "outPrefix",
        "inExclude",
        "outExclude",
    ])?;
    let defaults = ExtractPathsOptions::default();
    let length = |name: &str| -> Result<u32, GlueError> {
        Ok(o.count(name, 0)?
            .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX)))
    };
    Ok(ExtractPathsOptions {
        max_strings: o.int("maxStrings", 0)?.unwrap_or(defaults.max_strings),
        nbest: o.int("nbest", 0)?.unwrap_or(defaults.nbest),
        random: o.int("random", 0)?.unwrap_or(defaults.random),
        cycles: o.int("cycles", 0)?.unwrap_or(defaults.cycles),
        beam: o.weight("beam")?,
        obey_flags: o.flag("obeyFlags", false)?,
        print_flags: o.flag("printFlags", false)?,
        max_in_length: length("maxInLength")?,
        max_out_length: length("maxOutLength")?,
        in_prefix: o.text("inPrefix")?.unwrap_or_default(),
        out_prefix: o.text("outPrefix")?.unwrap_or_default(),
        in_exclude: o.text("inExclude")?.unwrap_or_default(),
        out_exclude: o.text("outExclude")?.unwrap_or_default(),
    })
}

/// `guess`'s options, and the text of the model form file it may take.
pub fn guess(o: &Options<'_>) -> Result<(GuessOptions, Option<String>), GlueError> {
    o.only(&[
        "modelForms",
        "maxNumberOfGuesses",
        "maxNumberOfForms",
        "generateThreshold",
    ])?;
    let defaults = GuessOptions::default();
    let options = GuessOptions {
        max_number_of_guesses: o
            .usize("maxNumberOfGuesses", 0)?
            .unwrap_or(defaults.max_number_of_guesses),
        max_number_of_forms: o
            .usize("maxNumberOfForms", 0)?
            .unwrap_or(defaults.max_number_of_forms),
        generate_threshold: o
            .weight("generateThreshold")?
            .unwrap_or(defaults.generate_threshold),
    };
    Ok((options, o.text("modelForms")?))
}

pub fn lookup_report(o: &Options<'_>) -> Result<LookupOptions, GlueError> {
    o.only(&[
        "inputFormat",
        "epsilonFormat",
        "maxNumber",
        "cycles",
        "beam",
        "timeCutoff",
        "cascade",
        "obeyFlags",
        "showFlags",
    ])?;
    let defaults = LookupOptions::default();
    Ok(LookupOptions {
        input_format: o.choice("inputFormat", &INPUT_FORMATS)?.unwrap_or_default(),
        epsilon_format: o.text("epsilonFormat")?.unwrap_or_default(),
        max_number: o.usize("maxNumber", 0)?,
        cycles: o.usize("cycles", 0)?.unwrap_or(defaults.cycles),
        beam: o.weight("beam")?,
        time_cutoff: o.seconds("timeCutoff")?.unwrap_or(0.0),
        cascade: o.choice("cascade", &CASCADES)?.unwrap_or_default(),
        obey_flags: o.flag("obeyFlags", true)?,
        show_flags: o.flag("showFlags", false)?,
    })
}

pub fn flookup_report(o: &Options<'_>) -> Result<FlookupOptions, GlueError> {
    o.only(&[
        "inputFormat",
        "epsilonFormat",
        "cycles",
        "beam",
        "timeCutoff",
        "invert",
        "forceOl",
        "obeyFlags",
        "showFlags",
    ])?;
    let defaults = FlookupOptions::default();
    Ok(FlookupOptions {
        input_format: o.choice("inputFormat", &INPUT_FORMATS)?.unwrap_or_default(),
        epsilon_format: o.text("epsilonFormat")?.unwrap_or_default(),
        cycles: o.usize("cycles", 0)?.unwrap_or(defaults.cycles),
        beam: o.weight("beam")?,
        time_cutoff: o.seconds("timeCutoff")?.unwrap_or(0.0),
        invert: o.flag("invert", false)?,
        force_ol: o.flag("forceOl", false)?,
        obey_flags: o.flag("obeyFlags", true)?,
        show_flags: o.flag("showFlags", false)?,
    })
}

pub fn optimized_lookup_report(o: &Options<'_>) -> Result<OptimizedLookupOptions, GlueError> {
    o.only(&["unique", "analyses", "timeCutoff", "beam", "fast"])?;
    let defaults = OptimizedLookupOptions::default();
    Ok(OptimizedLookupOptions {
        unique: o.flag("unique", false)?,
        max_analyses: o.int("analyses", 0)?.unwrap_or(defaults.max_analyses),
        time_cutoff: o.seconds("timeCutoff")?.unwrap_or(0.0),
        beam: o.weight("beam")?.unwrap_or(defaults.beam),
        fast: o.flag("fast", false)?,
    })
}

pub fn pair_test(o: &Options<'_>) -> Result<PairTestOptions, GlueError> {
    o.only(&["negativeTest", "xeroxMode"])?;
    Ok(PairTestOptions {
        negative_test: o.flag("negativeTest", false)?,
        xerox_mode: o.flag("xeroxMode", false)?,
    })
}

pub fn grep(o: &Options<'_>) -> Result<GrepOptions, GlueError> {
    o.only(&[
        "fixedStrings",
        "color",
        "wordRegexp",
        "lineRegexp",
        "invertMatch",
        "maxCount",
    ])?;
    Ok(GrepOptions {
        fixed_strings: o.flag("fixedStrings", false)?,
        color: o.flag("color", false)?,
        word_regexp: o.flag("wordRegexp", false)?,
        line_regexp: o.flag("lineRegexp", false)?,
        invert_match: o.flag("invertMatch", false)?,
        max_count: o.count("maxCount", 0)?.unwrap_or(0),
    })
}

/// `hfst pmatch`'s options. Those left out of the container's own settings
/// keep the container's.
pub fn pmatch(o: &Options<'_>) -> Result<PmatchOptions, GlueError> {
    o.only(&[
        "newline",
        "locate",
        "extractPatterns",
        "deletePatterns",
        "markPatterns",
        "maxContext",
        "maxRecursion",
        "timeCutoff",
        "weightCutoff",
    ])?;
    let defaults = PmatchOptions::default();
    Ok(PmatchOptions {
        blankline_separated: !o.flag("newline", false)?,
        locate: o.flag("locate", false)?,
        extract_patterns: o.maybe_flag("extractPatterns")?,
        delete_patterns: o.maybe_flag("deletePatterns")?,
        mark_patterns: o.maybe_flag("markPatterns")?,
        max_context: o.usize("maxContext", 0)?,
        max_recursion: o.usize("maxRecursion", 0)?,
        time_cutoff: o.seconds("timeCutoff")?.unwrap_or(0.0),
        weight_cutoff: o
            .number("weightCutoff", "a number", |n| !n.is_nan())?
            .map_or(defaults.weight_cutoff, |n| n as f32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::given;

    fn none() -> Options<'static> {
        given("query", &[])
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn options_left_out_are_the_tools_defaults() {
        assert!(!summarize(&none()).expect("none"));
        let lookup = lookup_report(&none()).expect("none");
        assert_eq!(lookup.cycles, 5);
        assert!(lookup.obey_flags && !lookup.show_flags);
        assert_eq!(lookup.cascade, Cascade::Union);
        assert_eq!(lookup.input_format, LookupInput::Text);
        let flookup = flookup_report(&none()).expect("none");
        assert!(!flookup.invert && !flookup.force_ol && flookup.obey_flags);
        let optimized = optimized_lookup_report(&none()).expect("none");
        let defaults = OptimizedLookupOptions::default();
        assert_eq!(optimized.max_analyses, defaults.max_analyses);
        assert_eq!(optimized.beam, defaults.beam);
        let paths = extract_paths(&none()).expect("none");
        assert_eq!(paths.cycles, ExtractPathsOptions::default().cycles);
        assert_eq!(paths.beam, None);
        let (guess, forms) = guess(&none()).expect("none");
        assert_eq!(guess.max_number_of_guesses, 5);
        assert_eq!(forms, None);
        let pmatch = pmatch(&none()).expect("none");
        assert!(pmatch.blankline_separated && !pmatch.locate);
        assert_eq!(pmatch.extract_patterns, None, "the container's own");
        assert_eq!(grep(&none()).expect("none").max_count, 0);
    }

    // [spec:hfst:req:javascript.compile-module/test]
    #[test]
    fn options_are_the_tools_flags_in_camel_case() {
        let o = given(
            "lookupReport",
            &[
                ("cascade", "priority-union".into()),
                ("inputFormat", "apertium".into()),
                ("timeCutoff", 1500.0.into()),
                ("maxNumber", 3.0.into()),
            ],
        );
        let lookup = lookup_report(&o).expect("lookup");
        assert_eq!(lookup.cascade, Cascade::PriorityUnion);
        assert_eq!(lookup.input_format, LookupInput::Apertium);
        assert_eq!(lookup.time_cutoff, 1.5, "milliseconds become seconds");
        assert_eq!(lookup.max_number, Some(3));
        let o = given(
            "flookupReport",
            &[("invert", true.into()), ("forceOl", true.into())],
        );
        let flookup = flookup_report(&o).expect("flookup");
        assert!(flookup.invert && flookup.force_ol);
        let o = given(
            "extractPaths",
            &[("nbest", 2.0.into()), ("inPrefix", "c".into())],
        );
        let paths = extract_paths(&o).expect("paths");
        assert_eq!((paths.nbest, paths.in_prefix.as_str()), (2, "c"));
        let o = given(
            "grep",
            &[("invertMatch", true.into()), ("fixedStrings", true.into())],
        );
        let grep = grep(&o).expect("grep");
        assert!(grep.invert_match && grep.fixed_strings);
        let o = given(
            "pmatch",
            &[("newline", true.into()), ("markPatterns", false.into())],
        );
        let pmatch = pmatch(&o).expect("pmatch");
        assert!(!pmatch.blankline_separated);
        assert_eq!(pmatch.mark_patterns, Some(false));
        let o = given("pairTest", &[("negativeTest", true.into())]);
        assert!(pair_test(&o).expect("pair test").negative_test);
        let o = given("compare", &[("eliminateFlags", true.into())]);
        assert!(compare(&o).expect("compare").eliminate_flags);
    }

    #[test]
    fn values_the_tools_turn_down_are_refused() {
        let o = given("lookupReport", &[("cascade", "intersection".into())]);
        assert!(lookup_report(&o).is_err());
        let o = given("optimizedLookupReport", &[("timeCutoff", (-5.0).into())]);
        assert!(optimized_lookup_report(&o).is_err());
        let o = given("grep", &[("maxCount", 1.5.into())]);
        assert!(grep(&o).is_err());
        let o = given("summarize", &[("verbos", true.into())]);
        let refused = summarize(&o).expect_err("misspelt");
        assert!(
            refused.message().contains("verbos"),
            "{}",
            refused.message()
        );
    }
}
