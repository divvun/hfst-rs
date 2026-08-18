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
//! those pairs were already the adaptation layer between the uneven per-backend
//! wrapper signatures, so they move here verbatim. An impl ignores arguments
//! its backend never used.

use crate::backend_ol::{ol_counts, ol_has_weights};
use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_data_types::Symbol;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_data_types::{
    HfstOneLevelPaths, HfstTwoLevelPaths, StringPair, StringPairSet, StringPairVector, StringVector,
};
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_ol_transducer::HfstOlTransducer;
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_transducer::{FlagDiacriticOverlay, decode_flag, encode_flag};
use crate::transducer::{Transducer, UnweightedTables, WeightedTables};
use crate::tropical_weight_transducer::TropicalWeightTransducer;
use hfst_openfst::StdVectorFst;

/// Binary algebra operation that may consume a virtual flag overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlagDiacriticOperation {
    Compose,
    Intersect,
    Subtract,
}

/// The reserved-symbol guard shared by every `encode_flag_diacritics` path: an
/// alphabet symbol already wrapped in '%...%' whose '@...@'-unescaped form is a
/// flag diacritic would collide with a freshly-encoded flag, so the C++ raised
/// an error. Kept as a `panic_any` (the port's throw port) so both the basic
/// round-trip default and the tropical in-place override fail identically.
// [spec:hfst:sem:hfst-transducer.hfst.encode-flag-diacritics-fn]
pub(crate) fn check_reserved_flag_collision(symbol: &str) {
    if symbol.len() > 4 {
        let bytes = symbol.as_bytes();
        if (bytes[0] == b'%') && (bytes[symbol.len() - 1] == b'%') {
            let mut str_bytes = bytes.to_vec();
            let last = str_bytes.len() - 1;
            str_bytes[0] = b'@';
            str_bytes[last] = b'@';
            let str = String::from_utf8(str_bytes)
                .expect("alphabet symbol remains valid UTF-8 after @-unescaping");
            if FdOperation::is_diacritic(&str) {
                let msg = "error: reserved symbol '".to_string() + &str + "' detected";
                std::panic::panic_any(msg);
            }
        }
    }
}

/// The surface every backend provides: identity, serialization tag, deep
/// copy, the typed conversions to/from the interchange
/// ['HfstBasicTransducer'], and the queries the C++ facade answered for every
/// backend (alphabet, cyclicity, path extraction, counts).
pub trait Backend: Sized {
    /// The stream/CLI tag this backend serializes as ('type' in the C++
    /// header). For the OL backends this is the LOGICAL type; the physical
    /// weightedness of loaded tables is the type parameter itself.
    const TYPE: ImplementationType;

    /// An empty transducer of this backend ('create_empty_transducer').
    /// Also replaces the C++ facade's UNSPECIFIED placeholder state — a
    /// facade without a backend is no longer representable.
    fn empty() -> Self;

    /// A deep copy ('HfstTransducer(const HfstTransducer&)' backend arm).
    fn copy(&self) -> crate::error::Result<Self>;

    /// The typed conversion to the interchange transducer — the per-type arms
    /// of 'get_basic_transducer' / 'hfst_transducer_to_hfst_basic_transducer'.
    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer>;

    /// The typed conversion from the interchange transducer — the per-type
    /// arms of 'HfstTransducer(const HfstBasicTransducer&, type)'.
    fn from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self>;

    /// [`Self::from_basic`] for a caller that is finished with `net`. A backend
    /// that can release the source as it consumes it overrides this to halve
    /// what the conversion of a very large graph stands at; the default keeps
    /// both alive, as the borrowing form must.
    fn from_basic_owned(net: HfstBasicTransducer) -> crate::error::Result<Self> {
        let converted = Self::from_basic(&net);
        drop(net);
        converted
    }

    /// 'get_alphabet' (every backend had a real arm).
    fn get_alphabet(&self) -> StringSet;

    /// 'is_cyclic' (every backend had a real arm).
    fn is_cyclic(&self) -> bool;

    /// 'number_of_states'. Deliberately undefaulted: a count is a fact the
    /// caller prints as one, and a stubbed 0 is indistinguishable at the call
    /// site from an honestly empty net. A backend that truly cannot count has
    /// to say so in its own body rather than inherit silence.
    fn number_of_states(&self) -> u32;

    /// 'number_of_arcs', undefaulted for the same reason as
    /// [`Self::number_of_states`].
    fn number_of_arcs(&self) -> u32;

    /// 'print_alphabet' (test function): tropical-only, no-op elsewhere.
    fn print_alphabet(&self) {}

    /// 'has_weights': does this transducer carry a non-zero weight anywhere?
    /// A content question, not a representation one — a weighted-SHAPED table
    /// whose every weight is 0.0 answers false, same as the equivalent tropical
    /// net would. Undefaulted for the reason [`Self::number_of_states`] is: an
    /// inherited false reads at the call site as a surveyed answer.
    fn has_weights(&self) -> bool;

    /// 'insert_to_alphabet(string)': the OL backends insert directly; the
    /// OpenFst backends round-trip through the basic transducer (the C++
    /// convert_to_basic_transducer / convert_to_hfst_transducer pair).
    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        let mut net = self.to_basic()?;
        net.add_symbol_to_alphabet(&crate::hfst_data_types::Symbol::new(symbol));
        *self = Self::from_basic(&net)?;
        Ok(())
    }

    /// 'insert_to_alphabet(StringSet)': like [`Self::insert_to_alphabet`] but for
    /// a whole set. Backends whose alphabet is graph-independent metadata (the
    /// tropical SymbolTable) override this with an in-place O(alphabet) edit; the
    /// default round-trips through the basic transducer as the C++ did.
    fn add_symbols_to_alphabet(&mut self, symbols: &StringSet) -> crate::error::Result<()> {
        let mut net = self.to_basic()?;
        net.add_symbols_to_alphabet_set(symbols);
        *self = Self::from_basic(&net)?;
        Ok(())
    }

    /// 'remove_from_alphabet(string)'. Default round-trips; the tropical backend
    /// edits its SymbolTable in place (no whole-graph deep copy).
    fn remove_from_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        let mut net = self.to_basic()?;
        net.remove_symbol_from_alphabet(&crate::hfst_data_types::Symbol::new(symbol));
        *self = Self::from_basic(&net)?;
        Ok(())
    }

    /// 'prune_alphabet'. Default round-trips; the tropical backend prunes its
    /// SymbolTable in place, keeping surviving symbols at their original labels.
    fn prune_alphabet(&mut self, force: bool) -> crate::error::Result<()> {
        let mut net = self.to_basic()?;
        net.prune_alphabet(force);
        *self = Self::from_basic(&net)?;
        Ok(())
    }

    /// The xerox-composition flag encode ('encode_flag_diacritics'): rewrite
    /// every flag-diacritic symbol '@...@' to its escaped form '%...%' so the
    /// product treats it as an ordinary symbol. The default rebuilds the whole
    /// graph through the basic transducer, which renumbers symbols in arc-walk
    /// order (the C++ behaviour); the tropical backend overrides this with an
    /// in-place SymbolTable rename that keeps each symbol at its original label
    /// (a pure alphabet edit — the arc graph never changes). Panics via
    /// 'panic_any' if the alphabet already carries a symbol whose escaped form
    /// would collide with an encoded flag (the reserved-symbol guard).
    fn encode_flag_diacritics(&mut self) {
        let basic_fst = self
            .to_basic()
            .expect("to_basic on a valid backend cannot fail");
        let mut basic_fst_copy = HfstBasicTransducer::new();
        let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

        for (s, states) in basic_fst.state_vector.iter().enumerate() {
            let s = s as HfstState;
            for transition in states.iter() {
                let input_symbol = transition.get_input_symbol(basic_fst.coder());
                let output_symbol = transition.get_output_symbol(basic_fst.coder());
                let isym = if FdOperation::is_diacritic(&input_symbol) {
                    encode_flag(&input_symbol)
                } else {
                    input_symbol
                };
                let osym = if FdOperation::is_diacritic(&output_symbol) {
                    encode_flag(&output_symbol)
                } else {
                    output_symbol
                };
                let tr = HfstBasicTransition::new_symbols(
                    transition.get_target_state(),
                    isym,
                    osym,
                    transition.get_weight(),
                    basic_fst_copy.coder_mut(),
                );
                basic_fst_copy.add_transition(s, &tr, true);
            }

            if basic_fst.is_final_state(s) {
                basic_fst_copy.set_final_weight(
                    s,
                    &basic_fst
                        .get_final_weight(s)
                        .expect("state was confirmed final via is_final_state"),
                );
            }
        }

        // copy alphabet, encode all flags
        let alpha = basic_fst.get_alphabet().clone();
        for it in alpha.iter() {
            check_reserved_flag_collision(it);
            let mut symbol: Symbol = it.clone();
            if FdOperation::is_diacritic(&symbol) {
                symbol = encode_flag(&symbol);
            }
            basic_fst_copy.add_symbol_to_alphabet(&symbol);
        }

        *self =
            Self::from_basic(&basic_fst_copy).expect("from_basic on a valid backend cannot fail");
    }

    /// The xerox-composition flag decode ('decode_flag_diacritics'): the exact
    /// inverse of [`Self::encode_flag_diacritics`], restoring '%...%' back to
    /// '@...@'. Default round-trips through the basic transducer (renumbering);
    /// the tropical backend overrides with an in-place SymbolTable rename.
    fn decode_flag_diacritics(&mut self) {
        let basic_fst = self
            .to_basic()
            .expect("to_basic on a valid backend cannot fail");
        let mut basic_fst_copy = HfstBasicTransducer::new();
        let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

        for (s, states) in basic_fst.state_vector.iter().enumerate() {
            let s = s as HfstState;
            for transition in states.iter() {
                let input_symbol = transition.get_input_symbol(basic_fst.coder());
                let output_symbol = transition.get_output_symbol(basic_fst.coder());

                let mut istr = decode_flag(&input_symbol);
                if !FdOperation::is_diacritic(&istr) {
                    istr = input_symbol;
                }

                let mut ostr = decode_flag(&output_symbol);
                if !FdOperation::is_diacritic(&ostr) {
                    ostr = output_symbol;
                }

                let tr = HfstBasicTransition::new_symbols(
                    transition.get_target_state(),
                    istr,
                    ostr,
                    transition.get_weight(),
                    basic_fst_copy.coder_mut(),
                );
                basic_fst_copy.add_transition(s, &tr, true);
            }

            if basic_fst.is_final_state(s) {
                basic_fst_copy.set_final_weight(
                    s,
                    &basic_fst
                        .get_final_weight(s)
                        .expect("state was confirmed final via is_final_state"),
                );
            }
        }

        // copy alphabet, decode all flags
        let alpha = basic_fst.get_alphabet().clone();
        for it in alpha.iter() {
            let mut symbol: Symbol = decode_flag(it);
            if !FdOperation::is_diacritic(&symbol) {
                symbol = it.clone();
            }
            basic_fst_copy.add_symbol_to_alphabet(&symbol);
        }

        *self =
            Self::from_basic(&basic_fst_copy).expect("from_basic on a valid backend cannot fail");
    }

    /// Convert this backend to a weighted optimized-lookup transducer — the
    /// pmatch archive writer's per-member conversion. The default round-trips
    /// through the basic transducer as the C++ did; the tropical backend
    /// overrides the harmonizer case with a direct arc-table walk, because the
    /// basic intermediate costs ~150 B/arc and owned hfst-pmatch2fst's peak
    /// RSS on multi-million-arc TOP definitions.
    fn to_hfst_ol(
        &self,
        weighted: bool,
        options: &str,
        harmonizer: Option<&crate::transducer::Transducer>,
    ) -> crate::error::Result<crate::transducer::Transducer> {
        let net = self.to_basic()?;
        ConversionFunctions::hfst_basic_transducer_to_hfst_ol(&net, weighted, options, harmonizer)
    }

    /// 'is_infinitely_ambiguous': the OL backends override with a walk of
    /// their own tables, which answers the same question without building the
    /// interchange copy; everything else goes through the basic transducer, as
    /// the C++ default arm did.
    fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        // hfst::implementations::HfstBasicTransducer net(*this);
        // return net.is_infinitely_ambiguous();
        Ok(self.to_basic()?.is_infinitely_ambiguous())
    }

    /// The runtime serialization tag: 'Self::TYPE', except for the OL
    /// backends, where the payload header's Weighted flag carries the logical
    /// OL/OLW distinction (interim invariant of
    /// [dec:hfst:monomorphic-backends]: conversions build weighted-shaped
    /// tables even for HFST_OL output, so the tag is data, not type).
    fn stream_type(&self) -> ImplementationType {
        Self::TYPE
    }

    /// Serialize this backend's payload to 'os' — the body of the C++
    /// per-type '*OutputStream::write_transducer'. 'hfst_format' is consulted
    /// only by the tropical backend (its symbol-table handling differs when no
    /// HFST framing is written).
    fn write(&self, os: &mut dyn std::io::Write, hfst_format: bool) -> crate::error::Result<()>;

    /// Serialize this backend's payload to a directory container — the
    /// directory-format counterpart of 'write' used by the '.thfst' sink of
    /// 'HfstOutputStream'. Every byte-stream backend has no directory encoding,
    /// so the default errors; the THFST backend overrides it to delegate to its
    /// 'write_dir' serializer. This is how the generic
    /// 'HfstOutputStream::write<B: Backend>' reaches the directory writer
    /// without a runtime downcast.
    // [spec:hfst:def:thfst-backend.stream-io]
    fn write_to_dir(&self, _dir: &std::path::Path) -> crate::error::Result<()> {
        crate::bail!(
            StreamCannotBeWritten,
            "this backend has no directory serialization"
        )
    }

    /// 'extract_paths' dispatch arm (no flag-diacritic filtering).
    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32);

    /// 'extract_paths' dispatch arm with the backend's own flag-diacritic
    /// table (the facade fetched the per-backend FdTable before dispatching;
    /// the tables' key types differ per backend, so the fetch lives here).
    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    );
}

/// The mutable FST algebra (tropical): every operation of the former
/// HfstApply.cc functor pairs plus the binary ops. Each returns a fresh
/// backend (the C++ freed the old one and stored the new — the facade's
/// assignment does that implicitly).
pub trait AlgebraBackend: Backend {
    /// Legacy capability name for virtual flag composition.
    const SUPPORTS_FLAG_OVERLAY: bool = false;

    /// Whether this backend consumes virtual flag loops during composition.
    const SUPPORTS_VIRTUAL_FLAG_COMPOSE: bool = Self::SUPPORTS_FLAG_OVERLAY;

    /// Whether this backend consumes virtual flag loops during intersection.
    const SUPPORTS_VIRTUAL_FLAG_INTERSECTION: bool = false;

    /// Whether this backend consumes virtual flag loops during subtraction.
    const SUPPORTS_VIRTUAL_FLAG_SUBTRACTION: bool = false;

    // ----- unary (apply) -----
    fn remove_epsilons(&self) -> Self;
    // determinize/minimize consume the input: the facade always overwrites its
    // stored fst with the result and drops the old one, so cloning it (a full
    // O(states) copy on the ~1M-state lang-sma intermediate) is pure waste.
    fn determinize(self, encode_weights: bool) -> Self;
    fn minimize(self, encode_weights: bool) -> Self;
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

    /// Fallible, consuming entry for operations with an optional virtual flag
    /// overlay. Backends may retain their borrowed implementation when no
    /// overlay or resource policy needs specialized handling.
    // [spec:hfst:req:virtual-flag-algebra.backend-core]
    fn try_flag_operation_owned(
        self,
        another: Self,
        operation: FlagDiacriticOperation,
        flag_overlay: Option<&FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<Self> {
        let _ = memory_limit_bytes;
        if flag_overlay.is_some() {
            let operation = match operation {
                FlagDiacriticOperation::Compose => "composition",
                FlagDiacriticOperation::Intersect => "intersection",
                FlagDiacriticOperation::Subtract => "subtraction",
            };
            crate::bail!(
                Hfst,
                format!("this backend does not support virtual flag {operation}")
            );
        }
        Ok(match operation {
            FlagDiacriticOperation::Compose => self.compose(&another),
            FlagDiacriticOperation::Intersect => self.intersect(&another),
            FlagDiacriticOperation::Subtract => self.subtract(&another),
        })
    }

    // ----- construction (the define_transducer_* constructor arms) -----
    fn define_transducer_spv(spv: &StringPairVector) -> Self;
    fn define_transducer_sps(sps: &StringPairSet, cyclic: bool) -> Self;
    fn define_transducer_spsv(spsv: &[StringPairSet]) -> Self;
    fn define_transducer_symbol(symbol: &str) -> Self;
    fn define_transducer_symbol_pair(isymbol: &str, osymbol: &str) -> Self;

    // ----- queries -----
    /// 'compare' backend arm.
    fn are_equivalent(&self, another: &Self, encode_weights: bool) -> bool;
    fn is_automaton(&self) -> bool;
    fn get_initial_input_symbols(&self) -> StringSet;
    fn get_first_input_symbols(&self) -> StringSet;

    // ----- paths and weights -----
    fn n_best(&self, n: u32) -> Self;
    fn extract_random_paths(&self, results: &mut HfstTwoLevelPaths, max_num: i32);
    /// 'set_final_weights'.
    fn set_final_weights(&self, weight: f32, increment: bool) -> Self;
    fn push_labels(&self, to_initial_state: bool) -> Self;
    fn push_weights(&self, to_initial_state: bool) -> Self;
    fn transform_weights(&self, func: fn(f32) -> f32) -> Self;

    // ----- substitution -----
    /// The both-sides symbol-substitution fast path of
    /// 'HfstTransducer::substitute(string, string, bool, bool)': dead code
    /// ('if (false && ...)') for the tropical backend, which returns 'None' to
    /// send the facade down the generic basic-transducer path.
    fn substitute_symbol_fast(&self, old_symbol: &str, new_symbol: &str) -> Option<Self>;
    fn substitute_string_transducer(&self, old_symbol_pair: StringPair, transducer: &Self) -> Self;
    /// 'disjunct(spv)': tropical mutates in place.
    fn disjunct_spv(&mut self, spv: &StringPairVector);
}

// ---------------------------------------------------------------------------
// Tropical (openfst-tropical / rustfst StdVectorFst)
// ---------------------------------------------------------------------------

impl Backend for StdVectorFst {
    const TYPE: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;
    fn empty() -> Self {
        TropicalWeightTransducer::create_empty_transducer()
    }
    fn copy(&self) -> crate::error::Result<Self> {
        Ok(TropicalWeightTransducer::copy(self))
    }
    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(self, true)
    }
    fn from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(ConversionFunctions::hfst_basic_transducer_to_tropical_ofst(
            net,
        ))
    }
    fn from_basic_owned(net: HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(ConversionFunctions::basic_to_tropical_ofst_owned(net))
    }
    fn get_alphabet(&self) -> StringSet {
        TropicalWeightTransducer::get_alphabet(self)
    }
    fn is_cyclic(&self) -> bool {
        TropicalWeightTransducer::is_cyclic(self)
    }
    fn number_of_states(&self) -> u32 {
        TropicalWeightTransducer::number_of_states(self)
    }
    fn number_of_arcs(&self) -> u32 {
        TropicalWeightTransducer::number_of_arcs(self)
    }
    fn print_alphabet(&self) {
        TropicalWeightTransducer::print_alphabet(self)
    }
    fn has_weights(&self) -> bool {
        TropicalWeightTransducer::has_weights(self)
    }
    // Alphabet edits are pure SymbolTable metadata for the tropical backend — the
    // arc graph is untouched — so these override the round-trip default with an
    // in-place edit that avoids two O(states) whole-graph deep copies.
    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        TropicalWeightTransducer::insert_to_alphabet(self, symbol);
        Ok(())
    }
    fn add_symbols_to_alphabet(&mut self, symbols: &StringSet) -> crate::error::Result<()> {
        TropicalWeightTransducer::add_symbols_to_alphabet(self, symbols);
        Ok(())
    }
    fn remove_from_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        TropicalWeightTransducer::remove_from_alphabet(self, symbol);
        Ok(())
    }
    fn prune_alphabet(&mut self, force: bool) -> crate::error::Result<()> {
        TropicalWeightTransducer::prune_alphabet(self, force);
        Ok(())
    }
    // A flag encode/decode is a pure alphabet rename — the arc graph is untouched
    // — so these override the whole-graph basic round-trip with an in-place
    // SymbolTable edit that preserves each symbol's original label. Equivalent
    // automaton, but the serialized bytes diverge from the round-trip (which
    // renumbers symbols in arc-walk order) by design ([node:flag-encode-diverge]).
    fn encode_flag_diacritics(&mut self) {
        TropicalWeightTransducer::encode_flag_diacritics(self);
    }
    fn decode_flag_diacritics(&mut self) {
        TropicalWeightTransducer::decode_flag_diacritics(self);
    }
    fn to_hfst_ol(
        &self,
        weighted: bool,
        options: &str,
        harmonizer: Option<&crate::transducer::Transducer>,
    ) -> crate::error::Result<crate::transducer::Transducer> {
        match harmonizer {
            Some(h) => ConversionFunctions::tropical_ofst_to_hfst_ol(self, weighted, options, h),
            None => ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                &self.to_basic()?,
                weighted,
                options,
                None,
            ),
        }
    }
    fn write(&self, os: &mut dyn std::io::Write, hfst_format: bool) -> crate::error::Result<()> {
        TropicalWeightTransducer::write_transducer_to(self, os, hfst_format)
    }
    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        TropicalWeightTransducer::extract_paths(self, callback, cycles, None, false);
    }
    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        let t_tropical_ofst = TropicalWeightTransducer::get_flag_diacritics(self);
        TropicalWeightTransducer::extract_paths(
            self,
            callback,
            cycles,
            Some(&t_tropical_ofst),
            filter_fd,
        );
    }
}

impl AlgebraBackend for StdVectorFst {
    const SUPPORTS_FLAG_OVERLAY: bool = true;
    const SUPPORTS_VIRTUAL_FLAG_INTERSECTION: bool = true;

    fn remove_epsilons(&self) -> Self {
        TropicalWeightTransducer::remove_epsilons(self)
    }
    fn determinize(self, encode_weights: bool) -> Self {
        TropicalWeightTransducer::determinize(self, encode_weights)
    }
    fn minimize(self, encode_weights: bool) -> Self {
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
    fn try_flag_operation_owned(
        self,
        another: Self,
        operation: FlagDiacriticOperation,
        flag_overlay: Option<&FlagDiacriticOverlay>,
        memory_limit_bytes: Option<u64>,
    ) -> crate::error::Result<Self> {
        match operation {
            FlagDiacriticOperation::Compose => TropicalWeightTransducer::try_compose_owned(
                self,
                another,
                flag_overlay,
                memory_limit_bytes,
            ),
            FlagDiacriticOperation::Intersect => TropicalWeightTransducer::try_intersect_owned(
                self,
                another,
                flag_overlay,
                memory_limit_bytes,
            ),
            FlagDiacriticOperation::Subtract => {
                if flag_overlay.is_some() {
                    crate::bail!(
                        Hfst,
                        "this backend does not support virtual flag subtraction"
                    );
                }
                Ok(TropicalWeightTransducer::subtract(&self, &another))
            }
        }
    }

    fn define_transducer_spv(spv: &StringPairVector) -> Self {
        TropicalWeightTransducer::define_transducer_spv(spv)
    }
    fn define_transducer_sps(sps: &StringPairSet, cyclic: bool) -> Self {
        TropicalWeightTransducer::define_transducer_sps(sps, cyclic)
    }
    fn define_transducer_spsv(spsv: &[StringPairSet]) -> Self {
        TropicalWeightTransducer::define_transducer_spsv(spsv)
    }
    fn define_transducer_symbol(symbol: &str) -> Self {
        TropicalWeightTransducer::define_transducer_symbol(symbol)
    }
    fn define_transducer_symbol_pair(isymbol: &str, osymbol: &str) -> Self {
        TropicalWeightTransducer::define_transducer_symbol_pair(isymbol, osymbol)
    }

    fn are_equivalent(&self, another: &Self, encode_weights: bool) -> bool {
        TropicalWeightTransducer::are_equivalent(self, another, encode_weights)
    }
    fn is_automaton(&self) -> bool {
        TropicalWeightTransducer::is_automaton(self)
    }
    fn get_initial_input_symbols(&self) -> StringSet {
        TropicalWeightTransducer::get_initial_input_symbols(self)
    }
    fn get_first_input_symbols(&self) -> StringSet {
        TropicalWeightTransducer::get_first_input_symbols(self)
    }

    fn n_best(&self, n: u32) -> Self {
        TropicalWeightTransducer::n_best(self, n)
    }
    fn extract_random_paths(&self, results: &mut HfstTwoLevelPaths, max_num: i32) {
        TropicalWeightTransducer::extract_random_paths(self, results, max_num);
    }
    fn set_final_weights(&self, weight: f32, increment: bool) -> Self {
        TropicalWeightTransducer::set_final_weights(self, weight, increment)
    }
    fn push_labels(&self, to_initial_state: bool) -> Self {
        TropicalWeightTransducer::push_labels(self, to_initial_state)
    }
    fn push_weights(&self, to_initial_state: bool) -> Self {
        TropicalWeightTransducer::push_weights(self, to_initial_state)
    }
    fn transform_weights(&self, func: fn(f32) -> f32) -> Self {
        TropicalWeightTransducer::transform_weights(self, func)
    }

    fn substitute_symbol_fast(&self, _old_symbol: &str, _new_symbol: &str) -> Option<Self> {
        // do not use until substituted symbols are correctly erased from the
        // alphabet
        // (tropical fast path is dead code: 'if false && ...'.)
        None
    }
    fn substitute_string_transducer(&self, old_symbol_pair: StringPair, transducer: &Self) -> Self {
        TropicalWeightTransducer::substitute_string_transducer(self, old_symbol_pair, transducer)
    }
    fn disjunct_spv(&mut self, spv: &StringPairVector) {
        TropicalWeightTransducer::disjunct_spv(self, spv);
    }
}

// ---------------------------------------------------------------------------
// Optimized-lookup (the two table instantiations)
// ---------------------------------------------------------------------------

impl Backend for Transducer<WeightedTables> {
    const TYPE: ImplementationType = ImplementationType::HFST_OLW_TYPE;
    fn stream_type(&self) -> ImplementationType {
        // Weighted-shaped tables may carry a logically-unweighted transducer
        // (interim invariant); the header flag is the tag.
        if self.is_weighted() {
            ImplementationType::HFST_OLW_TYPE
        } else {
            ImplementationType::HFST_OL_TYPE
        }
    }
    fn write(&self, os: &mut dyn std::io::Write, _hfst_format: bool) -> crate::error::Result<()> {
        Transducer::write(self, os);
        Ok(())
    }
    fn empty() -> Self {
        Transducer::new_empty()
    }
    fn copy(&self) -> crate::error::Result<Self> {
        Transducer::copy(self)
    }
    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        Ok(ConversionFunctions::hfst_ol_to_hfst_basic_transducer(self))
    }
    fn from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        ConversionFunctions::hfst_basic_transducer_to_hfst_ol(net, true, "", None)
    }
    fn get_alphabet(&self) -> StringSet {
        HfstOlTransducer::get_alphabet(self)
    }
    fn is_cyclic(&self) -> bool {
        HfstOlTransducer::is_cyclic(self)
    }
    fn number_of_states(&self) -> u32 {
        ol_counts(self).0
    }
    fn number_of_arcs(&self) -> u32 {
        ol_counts(self).1
    }
    fn has_weights(&self) -> bool {
        ol_has_weights(self)
    }
    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        self.include_symbol_in_alphabet(symbol);
        Ok(())
    }
    fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        Ok(Transducer::is_infinitely_ambiguous(self))
    }
    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        HfstOlTransducer::extract_paths(self, callback, cycles, None, false);
    }
    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        let t_hfst_ol = HfstOlTransducer::get_flag_diacritics(self);
        HfstOlTransducer::extract_paths(self, callback, cycles, Some(t_hfst_ol), filter_fd);
        // don't delete t_hfst_ol, it's not a copy of the FdTable but the
        // real thing
    }
}

impl Backend for Transducer<UnweightedTables> {
    const TYPE: ImplementationType = ImplementationType::HFST_OL_TYPE;
    fn write(&self, os: &mut dyn std::io::Write, _hfst_format: bool) -> crate::error::Result<()> {
        Transducer::write(self, os);
        Ok(())
    }
    fn empty() -> Self {
        Transducer::new_empty()
    }
    fn copy(&self) -> crate::error::Result<Self> {
        Transducer::copy(self)
    }
    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        Ok(ConversionFunctions::hfst_ol_to_hfst_basic_transducer(self))
    }
    fn from_basic(_net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        // Interim invariant ([dec:hfst:monomorphic-backends]): conversions
        // always build weighted-shaped tables in memory (as the C++ did even
        // for HFST_OL_TYPE output); an unweighted-tables backend can only be
        // produced by a disk load.
        crate::bail!(
            Fatal,
            "from_basic: HFST_OL conversions produce weighted-shaped tables; \
             build Transducer<WeightedTables> instead"
        )
    }
    fn get_alphabet(&self) -> StringSet {
        HfstOlTransducer::get_alphabet(self)
    }
    fn is_cyclic(&self) -> bool {
        HfstOlTransducer::is_cyclic(self)
    }
    fn number_of_states(&self) -> u32 {
        ol_counts(self).0
    }
    fn number_of_arcs(&self) -> u32 {
        ol_counts(self).1
    }
    fn has_weights(&self) -> bool {
        ol_has_weights(self)
    }
    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        self.include_symbol_in_alphabet(symbol);
        Ok(())
    }
    fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        Ok(Transducer::is_infinitely_ambiguous(self))
    }
    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        HfstOlTransducer::extract_paths(self, callback, cycles, None, false);
    }
    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        let t_hfst_ol = HfstOlTransducer::get_flag_diacritics(self);
        HfstOlTransducer::extract_paths(self, callback, cycles, Some(t_hfst_ol), filter_fd);
        // don't delete t_hfst_ol, it's not a copy of the FdTable but the
        // real thing
    }
}

/// The lookup surface (OL backends). The underlying 'hfst_ol' lookup engine
/// mutates internal state, so these take '&mut self'; the facade exposes them
/// on '&self' through the const-cast island (see 'HfstTransducer').
pub trait LookupBackend: Backend {
    fn lookup_fd_str(&mut self, s: &str, limit: isize, time_cutoff: f64) -> HfstOneLevelPaths;
    fn lookup_fd_strvec(
        &mut self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstOneLevelPaths;
    fn lookup_fd_pairs_str(&mut self, s: &str, limit: isize, time_cutoff: f64)
    -> HfstTwoLevelPaths;
    fn is_lookup_infinitely_ambiguous_str(&mut self, s: &str) -> bool;
    fn is_lookup_infinitely_ambiguous_strvec(&mut self, s: &StringVector) -> bool;
}

macro_rules! ol_lookup_backend {
    ($tables:ty) => {
        impl LookupBackend for Transducer<$tables> {
            fn lookup_fd_str(
                &mut self,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> HfstOneLevelPaths {
                Transducer::lookup_fd_str(self, s, limit, time_cutoff)
            }
            fn lookup_fd_strvec(
                &mut self,
                s: &StringVector,
                limit: isize,
                time_cutoff: f64,
            ) -> HfstOneLevelPaths {
                Transducer::lookup_fd_strvec(self, s, limit, time_cutoff)
            }
            fn lookup_fd_pairs_str(
                &mut self,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> HfstTwoLevelPaths {
                Transducer::lookup_fd_pairs_str(self, s, limit, time_cutoff)
            }
            fn is_lookup_infinitely_ambiguous_str(&mut self, s: &str) -> bool {
                Transducer::is_lookup_infinitely_ambiguous_str(self, s)
            }
            fn is_lookup_infinitely_ambiguous_strvec(&mut self, s: &StringVector) -> bool {
                Transducer::is_lookup_infinitely_ambiguous_strvec(self, s)
            }
        }
    };
}

ol_lookup_backend!(WeightedTables);
ol_lookup_backend!(UnweightedTables);
