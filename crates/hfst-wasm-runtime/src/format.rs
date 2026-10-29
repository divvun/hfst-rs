//! The transducer formats the JavaScript API names.

use hfst::hfst_data_types::ImplementationType;

// [spec:hfst:req:javascript.load-bytes+1]
/// A transducer format, under the name `hfst fst2fst --format` gives it.
/// `js/shared/transducer.ts` and `js/hfst/transducer.ts` list the same names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    OptimizedLookupWeighted,
    OptimizedLookupUnweighted,
    OpenFstTropical,
    Foma,
}

impl Format {
    pub const ALL: [Format; 4] = [
        Format::OptimizedLookupWeighted,
        Format::OptimizedLookupUnweighted,
        Format::OpenFstTropical,
        Format::Foma,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Format::OptimizedLookupWeighted => "optimized-lookup-weighted",
            Format::OptimizedLookupUnweighted => "optimized-lookup-unweighted",
            Format::OpenFstTropical => "openfst-tropical",
            Format::Foma => "foma",
        }
    }

    pub fn from_name(name: &str) -> Option<Format> {
        Self::ALL.into_iter().find(|format| format.name() == name)
    }

    pub fn implementation_type(self) -> ImplementationType {
        match self {
            Format::OptimizedLookupWeighted => ImplementationType::HFST_OLW_TYPE,
            Format::OptimizedLookupUnweighted => ImplementationType::HFST_OL_TYPE,
            Format::OpenFstTropical => ImplementationType::TROPICAL_OPENFST_TYPE,
            Format::Foma => ImplementationType::FOMA_TYPE,
        }
    }

    /// The format of a transducer of implementation type `ty`, or `None` for
    /// a type the JavaScript API does not carry.
    pub fn of(ty: ImplementationType) -> Option<Format> {
        match ty {
            ImplementationType::HFST_OLW_TYPE => Some(Format::OptimizedLookupWeighted),
            ImplementationType::HFST_OL_TYPE => Some(Format::OptimizedLookupUnweighted),
            ImplementationType::TROPICAL_OPENFST_TYPE => Some(Format::OpenFstTropical),
            ImplementationType::FOMA_TYPE => Some(Format::Foma),
            ImplementationType::SFST_TYPE
            | ImplementationType::XFSM_TYPE
            | ImplementationType::THFST_TYPE
            | ImplementationType::HFST2_TYPE
            | ImplementationType::UNSPECIFIED_TYPE
            | ImplementationType::ERROR_TYPE => None,
        }
    }

    pub fn in_runtime(self) -> bool {
        match self {
            Format::OptimizedLookupWeighted | Format::OptimizedLookupUnweighted => true,
            Format::OpenFstTropical | Format::Foma => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_round_trip_through_name_and_type() {
        for format in Format::ALL {
            assert_eq!(Format::from_name(format.name()), Some(format));
            assert_eq!(Format::of(format.implementation_type()), Some(format));
        }
        assert_eq!(Format::from_name("thfst"), None);
        assert_eq!(Format::of(ImplementationType::THFST_TYPE), None);
    }

    #[test]
    fn the_wrapper_lists_every_format_name() {
        let read = |path: &str| {
            std::fs::read_to_string(format!("{}/../../js/{path}", env!("CARGO_MANIFEST_DIR")))
                .expect("the wrapper is in the repository")
        };
        let shared = read("shared/transducer.ts");
        let full = read("hfst/transducer.ts");
        for format in Format::ALL {
            let name = format!("\"{}\"", format.name());
            let file = if format.in_runtime() { &shared } else { &full };
            assert!(file.contains(&name), "the wrapper lists {name}");
        }
    }
}
