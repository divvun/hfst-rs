//! Lookup on every transducer type the full package carries. Optimized-lookup
//! transducers take the runtime's path. Tropical and foma transducers take
//! the general lookup `hfst lookup` runs on them, so their results are the
//! ones it prints.

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPaths, StringVector};
use hfst::hfst_lookup_format::{LookupInputFormat, is_possible_to_get_result, parse_lookup_line};
use hfst::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use hfst::hfst_symbol_defs::{StringSet, internal_identity, internal_unknown};
use hfst::hfst_transducer::AnyTransducer;
use hfst_wasm_runtime::{
    GlueError, LookupLimits, LookupResult, lookup_results, lookup_results_to_js,
};
use wasm_bindgen::prelude::*;

use crate::transducer::FullTransducer;

/// How many times `hfst lookup` follows an input epsilon cycle when the
/// lookup is infinitely ambiguous: its `--cycles` default.
const CYCLES: usize = 5;

/// What `hfst lookup` builds once per tropical or foma transducer before its
/// first lookup: the basic transducer it walks, the input symbols it has, and
/// a tokeniser that knows its multicharacter symbols.
pub(crate) struct GeneralLookup {
    basic: HfstBasicTransducer,
    symbols_seen: StringSet,
    unknown_or_identity_seen: bool,
    tokenizer: HfstStrings2FstTokenizer,
}

impl GeneralLookup {
    fn new(any: &AnyTransducer) -> Result<GeneralLookup, GlueError> {
        let basic = match any {
            AnyTransducer::Tropical(t) => {
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(t)?
            }
            AnyTransducer::Foma(t) => {
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(t)?
            }
            AnyTransducer::OlW(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_) => {
                return Err(GlueError::new(
                    "internal",
                    "the general lookup is for tropical and foma transducers",
                ));
            }
        };
        let mut symbols_seen = StringSet::new();
        for transitions in basic.iter() {
            for transition in transitions.iter() {
                symbols_seen.insert(transition.get_input_symbol(basic.coder()));
            }
        }
        let unknown_or_identity_seen =
            symbols_seen.contains(internal_unknown) || symbols_seen.contains(internal_identity);
        let multichar: StringVector = symbols_seen
            .iter()
            .filter(|symbol| symbol.chars().count() > 1)
            .cloned()
            .collect();
        let tokenizer = HfstStrings2FstTokenizer::new(&multichar, "")?;
        Ok(GeneralLookup {
            basic,
            symbols_seen,
            unknown_or_identity_seen,
            tokenizer,
        })
    }

    // [spec:hfst:req:javascript.lookup]
    /// The paths `hfst lookup` finds for `input`: the input tokenised as it
    /// tokenises a line, nothing looked up when a symbol is foreign to the
    /// transducer, flags obeyed, and input epsilon cycles followed `CYCLES`
    /// times when the lookup is infinitely ambiguous. `limit` caps the result
    /// count as the general lookup's own limit does, which `hfst lookup`
    /// leaves off; the general lookup has no time limit.
    fn lookup(&self, input: &str, limit: isize) -> Result<HfstOneLevelPaths, GlueError> {
        let mut line = input.to_owned();
        let mut markup = String::new();
        let mut outside_sigma = false;
        let path = parse_lookup_line(
            &mut line,
            &self.tokenizer,
            &mut markup,
            &mut outside_sigma,
            false,
            LookupInputFormat::Utf8TokenInput,
        )?;
        if !is_possible_to_get_result(&path, &self.symbols_seen, self.unknown_or_identity_seen) {
            return Ok(HfstOneLevelPaths::new());
        }
        let cycles = self
            .basic
            .is_lookup_infinitely_ambiguous_path(&path, true)
            .then_some(CYCLES);
        let limit = i32::try_from(limit).unwrap_or(i32::MAX);
        let mut pairs = HfstTwoLevelPaths::new();
        self.basic
            .lookup(&path.second, &mut pairs, cycles, None, limit, true);
        Ok(pairs
            .into_iter()
            .map(|pair| HfstOneLevelPath {
                first: pair.first,
                second: pair.second.into_iter().map(|(_, output)| output).collect(),
            })
            .collect())
    }
}

#[wasm_bindgen]
impl FullTransducer {
    // [spec:hfst:req:javascript.lookup]
    /// Every result of looking `input` up within the limits, as the runtime
    /// orders them.
    pub fn lookup(
        &self,
        input: &str,
        limit: Option<u32>,
        time_limit_ms: Option<f64>,
    ) -> Result<JsValue, GlueError> {
        let limits = LookupLimits::new(limit, time_limit_ms)?;
        Ok(lookup_results_to_js(self.lookup_results(input, limits)?))
    }
}

impl FullTransducer {
    fn lookup_results(
        &self,
        input: &str,
        limits: LookupLimits,
    ) -> Result<Vec<LookupResult>, GlueError> {
        let paths = match &self.any {
            AnyTransducer::OlW(t) => {
                t.lookup_state()
                    .lookup_fd(input, limits.results, limits.seconds)
            }
            AnyTransducer::OlU(t) => {
                t.lookup_state()
                    .lookup_fd(input, limits.results, limits.seconds)
            }
            AnyTransducer::Tropical(_) | AnyTransducer::Foma(_) => {
                self.general_lookup()?.lookup(input, limits.results)?
            }
            AnyTransducer::Thfst(_) => {
                return Err(GlueError::new(
                    "internal",
                    "a THFST transducer reached the full package, which does not carry the type",
                ));
            }
        };
        Ok(lookup_results(paths))
    }

    fn general_lookup(&self) -> Result<&GeneralLookup, GlueError> {
        if let Some(general) = self.general.get() {
            return Ok(general);
        }
        let general = GeneralLookup::new(&self.any)?;
        Ok(self.general.get_or_init(|| general))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transducer::{read_any_transducers, read_att};
    use hfst_wasm_runtime::Format;

    /// A path through `@P.CASE.GEN@ t @R.CASE.NOM@` that the flags block,
    /// two they allow, and a multicharacter output symbol.
    const ATT: &str = "\
0\t1\t@P.CASE.NOM@\t@P.CASE.NOM@\t0.5
0\t2\t@P.CASE.GEN@\t@P.CASE.GEN@
1\t3\tt\tt
2\t4\tt\tt
3\t5\t@R.CASE.NOM@\t@R.CASE.NOM@
4\t5\t@R.CASE.NOM@\t@R.CASE.NOM@
4\t5\t@R.CASE.GEN@\t@R.CASE.GEN@\t1.5
5\t6\t@0@\t+N
6
";

    fn one(transducers: Result<Vec<FullTransducer>, GlueError>) -> FullTransducer {
        let mut all = transducers.expect("reads");
        assert_eq!(all.len(), 1);
        all.remove(0)
    }

    /// The transducer as it loads from bytes. An optimized-lookup-unweighted
    /// transducer converted in memory keeps its weights until it is written,
    /// as the library builds every optimized-lookup table weighted.
    fn transducer(format: Format) -> FullTransducer {
        let mut converted = one(read_att(ATT, format.name()));
        one(read_any_transducers(&converted.to_bytes().expect("writes")))
    }

    fn summary(results: &[LookupResult]) -> Vec<String> {
        results
            .iter()
            .map(|r| format!("{} {}", r.weight, r.symbols.join(" ")))
            .collect()
    }

    fn no_limits() -> LookupLimits {
        LookupLimits::new(None, None).expect("no limits")
    }

    #[test]
    fn every_type_obeys_the_flags() {
        for format in Format::ALL {
            let t = transducer(format);
            let results = t.lookup_results("t", no_limits()).expect("looks up");
            let weighted = matches!(
                format,
                Format::OptimizedLookupWeighted | Format::OpenFstTropical
            );
            let expected = if weighted {
                ["0.5 t +N", "1.5 t +N"]
            } else {
                ["0 t +N", "0 t +N"]
            };
            assert_eq!(summary(&results), expected, "{format:?}");
        }
    }

    #[test]
    fn a_limit_caps_every_type() {
        for format in Format::ALL {
            let t = transducer(format);
            let limits = LookupLimits::new(Some(1), None).expect("a limit");
            let results = t.lookup_results("t", limits).expect("looks up");
            assert_eq!(results.len(), 1, "{format:?}");
            let none = LookupLimits::new(Some(0), None).expect("a zero limit");
            assert!(t.lookup_results("t", none).expect("looks up").is_empty());
        }
    }

    #[test]
    fn foreign_and_empty_input_have_no_results() {
        for format in Format::ALL {
            let t = transducer(format);
            for input in ["", "x", "tt", "ø"] {
                let results = t.lookup_results(input, no_limits()).expect("looks up");
                assert!(results.is_empty(), "{format:?} {input:?}: {results:?}");
            }
        }
    }

    #[test]
    fn the_general_lookup_is_built_once() {
        let t = transducer(Format::OpenFstTropical);
        assert!(t.general.get().is_none());
        t.lookup_results("t", no_limits()).expect("looks up");
        let first: *const GeneralLookup = t.general_lookup().expect("built");
        t.lookup_results("t", no_limits()).expect("looks up again");
        assert!(std::ptr::eq(first, t.general_lookup().expect("kept")));
    }
}
