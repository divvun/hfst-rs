//! The transducer formats a stream member can declare.

// [spec:hfst:req:javascript.load-bytes+2]
/// A transducer format, under the name `hfst fst2fst --format` gives it. The
/// package reads the optimized-lookup formats, which `js/hfst/transducer.ts`
/// lists, and turns down the others by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    OptimizedLookupWeighted,
    OptimizedLookupUnweighted,
    OpenFstTropical,
    Foma,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::OptimizedLookupWeighted => "optimized-lookup-weighted",
            Format::OptimizedLookupUnweighted => "optimized-lookup-unweighted",
            Format::OpenFstTropical => "openfst-tropical",
            Format::Foma => "foma",
        }
    }

    pub fn is_read(self) -> bool {
        match self {
            Format::OptimizedLookupWeighted | Format::OptimizedLookupUnweighted => true,
            Format::OpenFstTropical | Format::Foma => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Format; 4] = [
        Format::OptimizedLookupWeighted,
        Format::OptimizedLookupUnweighted,
        Format::OpenFstTropical,
        Format::Foma,
    ];

    #[test]
    fn the_wrapper_lists_exactly_the_formats_read() {
        let wrapper = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../js/hfst/transducer.ts"
        ))
        .expect("the wrapper is in the repository");
        for format in ALL {
            let name = format!("\"{}\"", format.name());
            assert_eq!(
                wrapper.contains(&name),
                format.is_read(),
                "the wrapper lists {name} exactly when the package reads it"
            );
        }
    }
}
