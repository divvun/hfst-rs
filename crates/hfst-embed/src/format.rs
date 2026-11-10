//! The transducer formats a stream member can declare.

// [spec:hfst:req:javascript.load-bytes+2]
/// A transducer format, under the name `hfst fst2fst --format` gives it. The
/// packages read the optimized-lookup formats and turn down the others by
/// name.
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

    pub fn is_read(self) -> bool {
        match self {
            Format::OptimizedLookupWeighted | Format::OptimizedLookupUnweighted => true,
            Format::OpenFstTropical | Format::Foma => false,
        }
    }
}
