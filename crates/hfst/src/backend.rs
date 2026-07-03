//! The monomorphic backend taxonomy — [dec:hfst:monomorphic-backends].
//!
//! The C++ facade dispatched every operation over a runtime type tag
//! (`union` + `ImplementationType`, ported as the `TransducerImplementation`
//! enum). Here each backend is a type parameter of the facade and each
//! operation is a trait method, so the whole library monomorphizes; the only
//! runtime type decision left is at the stream/format boundary where file
//! bytes enter the program.
//!
//! The method bodies are the facade's former per-backend closure pairs (the
//! `apply`/`apply_bool`/`apply_n`/`apply_binary` functors of HfstApply.cc) —
//! those pairs were already the adaptation layer between the uneven
//! `TropicalWeightTransducer`/`LogWeightTransducer` wrapper signatures, so
//! they move here verbatim. An impl ignores arguments its backend never used
//! (e.g. the log backend ignores `encode_weights`, exactly as its closure
//! did).

use crate::hfst_data_types::ImplementationType;
use crate::log_weight_transducer::{LogFst, LogWeightTransducer};
use crate::tropical_weight_transducer::TropicalWeightTransducer;
use hfst_openfst::StdVectorFst;

/// The surface every backend provides: identity + serialization tag.
pub trait Backend: Sized {
    /// The stream/CLI tag this backend serializes as ('type' in the C++
    /// header). For the OL backends this is the LOGICAL type; the physical
    /// weightedness of loaded tables is the type parameter itself.
    const TYPE: ImplementationType;
}

/// The mutable FST algebra (tropical + log): every operation of the former
/// HfstApply.cc functor pairs plus the binary ops. Each returns a fresh
/// backend (the C++ freed the old one and stored the new — the facade's
/// assignment does that implicitly).
pub trait AlgebraBackend: Backend {
    // ----- unary (apply) -----
    fn remove_epsilons(&self) -> Self;
    fn determinize(&self, encode_weights: bool) -> Self;
    fn minimize(&self, encode_weights: bool) -> Self;
    fn repeat_star(&self) -> Self;
    fn repeat_plus(&self) -> Self;
    fn repeat_n(&self, n: u32) -> Self;
    fn repeat_le_n(&self, n: u32) -> Self;
    fn optionalize(&self) -> Self;
    fn invert(&self) -> Self;
    fn reverse(&self) -> Self;
    fn extract_input_language(&self) -> Self;
    fn extract_output_language(&self) -> Self;

    // ----- binary (apply_binary / apply_another) -----
    fn concatenate(&self, another: &Self) -> Self;
    fn disjunct(&self, another: &Self) -> Self;
    fn intersect(&self, another: &Self) -> Self;
    fn subtract(&self, another: &Self) -> Self;
    fn compose(&self, another: &Self) -> Self;
}

// ---------------------------------------------------------------------------
// Tropical (openfst-tropical / rustfst StdVectorFst)
// ---------------------------------------------------------------------------

impl Backend for StdVectorFst {
    const TYPE: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;
}

impl AlgebraBackend for StdVectorFst {
    fn remove_epsilons(&self) -> Self {
        TropicalWeightTransducer::remove_epsilons(self)
    }
    fn determinize(&self, encode_weights: bool) -> Self {
        TropicalWeightTransducer::determinize(self, encode_weights)
    }
    fn minimize(&self, encode_weights: bool) -> Self {
        TropicalWeightTransducer::minimize(self, encode_weights)
    }
    fn repeat_star(&self) -> Self {
        TropicalWeightTransducer::repeat_star(self)
    }
    fn repeat_plus(&self) -> Self {
        TropicalWeightTransducer::repeat_plus(self)
    }
    fn repeat_n(&self, n: u32) -> Self {
        TropicalWeightTransducer::repeat_n(self, n)
    }
    fn repeat_le_n(&self, n: u32) -> Self {
        TropicalWeightTransducer::repeat_le_n(self, n)
    }
    fn optionalize(&self) -> Self {
        TropicalWeightTransducer::optionalize(self)
    }
    fn invert(&self) -> Self {
        TropicalWeightTransducer::invert(self)
    }
    fn reverse(&self) -> Self {
        TropicalWeightTransducer::reverse(self)
    }
    fn extract_input_language(&self) -> Self {
        TropicalWeightTransducer::extract_input_language(self)
    }
    fn extract_output_language(&self) -> Self {
        TropicalWeightTransducer::extract_output_language(self)
    }

    fn concatenate(&self, another: &Self) -> Self {
        TropicalWeightTransducer::concatenate(self, another)
    }
    fn disjunct(&self, another: &Self) -> Self {
        TropicalWeightTransducer::disjunct(self, another)
    }
    fn intersect(&self, another: &Self) -> Self {
        TropicalWeightTransducer::intersect(self, another)
    }
    fn subtract(&self, another: &Self) -> Self {
        TropicalWeightTransducer::subtract(self, another)
    }
    fn compose(&self, another: &Self) -> Self {
        TropicalWeightTransducer::compose(self, another)
    }
}

// ---------------------------------------------------------------------------
// Log (openfst-log)
// ---------------------------------------------------------------------------

impl Backend for LogFst {
    const TYPE: ImplementationType = ImplementationType::LOG_OPENFST_TYPE;
}

impl AlgebraBackend for LogFst {
    fn remove_epsilons(&self) -> Self {
        LogWeightTransducer::remove_epsilons(self)
    }
    fn determinize(&self, _encode_weights: bool) -> Self {
        // The log backend never encoded weights (its closure ignored the flag).
        LogWeightTransducer::determinize(self)
    }
    fn minimize(&self, _encode_weights: bool) -> Self {
        LogWeightTransducer::minimize(self)
    }
    fn repeat_star(&self) -> Self {
        LogWeightTransducer::repeat_star(self)
    }
    fn repeat_plus(&self) -> Self {
        LogWeightTransducer::repeat_plus(self)
    }
    fn repeat_n(&self, n: u32) -> Self {
        LogWeightTransducer::repeat_n(self, n)
    }
    fn repeat_le_n(&self, n: u32) -> Self {
        LogWeightTransducer::repeat_le_n(self, n)
    }
    fn optionalize(&self) -> Self {
        LogWeightTransducer::optionalize(self)
    }
    fn invert(&self) -> Self {
        LogWeightTransducer::invert(self)
    }
    fn reverse(&self) -> Self {
        LogWeightTransducer::reverse(self)
    }
    fn extract_input_language(&self) -> Self {
        LogWeightTransducer::extract_input_language(self)
    }
    fn extract_output_language(&self) -> Self {
        LogWeightTransducer::extract_output_language(self)
    }

    fn concatenate(&self, another: &Self) -> Self {
        LogWeightTransducer::concatenate(self, another)
    }
    fn disjunct(&self, another: &Self) -> Self {
        LogWeightTransducer::disjunct(self, another)
    }
    fn intersect(&self, another: &Self) -> Self {
        LogWeightTransducer::intersect(self, another)
    }
    fn subtract(&self, another: &Self) -> Self {
        LogWeightTransducer::subtract(self, another)
    }
    fn compose(&self, another: &Self) -> Self {
        LogWeightTransducer::compose(self, another)
    }
}

// ---------------------------------------------------------------------------
// Optimized-lookup (the two table instantiations)
// ---------------------------------------------------------------------------

impl Backend for crate::transducer::Transducer<crate::transducer::WeightedTables> {
    const TYPE: ImplementationType = ImplementationType::HFST_OLW_TYPE;
}

impl Backend for crate::transducer::Transducer<crate::transducer::UnweightedTables> {
    const TYPE: ImplementationType = ImplementationType::HFST_OL_TYPE;
}

/// The lookup surface (OL backends). Populated as the facade rewrite reaches
/// the lookup methods.
pub trait LookupBackend: Backend {}

impl LookupBackend for crate::transducer::Transducer<crate::transducer::WeightedTables> {}
impl LookupBackend for crate::transducer::Transducer<crate::transducer::UnweightedTables> {}
