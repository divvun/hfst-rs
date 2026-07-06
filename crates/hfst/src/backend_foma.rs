//! Native foma backend — makes `ImplementationType::FOMA_TYPE` a real,
//! usable transducer implementation, backed by the standalone Rust port of
//! foma (the `foma` crate, a path dependency).
//!
//! The whole module is gated behind the `foma` Cargo feature (see the module
//! declaration in `lib.rs`); with the feature off, nothing here compiles and
//! the facade behaves exactly as before (FOMA_TYPE stays unavailable). The
//! upstream C++ `FomaTransducer.*` / `ConvertFomaTransducer.*` are excluded
//! from the source-impl scope, so these rules are authored greenfield against
//! the contract in `docs/spec/port/back-ends/foma/foma-backend.md`.

use crate::backend::Backend;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_tropical_transducer_transition_data::{SymbolType, WeightType};

use foma::types::Sigma;

/// The HFST special-symbol strings for foma's three reserved sigma numbers.
const EPSILON_SYMBOL: &str = "@_EPSILON_SYMBOL_@";
const UNKNOWN_SYMBOL: &str = "@_UNKNOWN_SYMBOL_@";
const IDENTITY_SYMBOL: &str = "@_IDENTITY_SYMBOL_@";

/// A newtype wrapper over foma's `Fsm` — the backend's transducer handle. The
/// inner `Fsm` is foma's sentinel-terminated line table plus its `Sigma`
/// alphabet (number<->symbol, with reserved numbers EPSILON=0, UNKNOWN=1,
/// IDENTITY=2).
// [spec:hfst:def:foma-backend.foma-transducer]
#[derive(Clone, Debug)]
pub struct FomaTransducer(pub foma::types::Fsm);

/// Map a foma sigma number to its HFST symbol string. The three reserved
/// numbers map to their HFST special strings; every other number is resolved
/// through the sigma alphabet.
fn sym(n: i32, sigma: Option<&Sigma>) -> SymbolType {
    match n {
        foma::types::EPSILON => SymbolType::from(EPSILON_SYMBOL),
        foma::types::UNKNOWN => SymbolType::from(UNKNOWN_SYMBOL),
        foma::types::IDENTITY => SymbolType::from(IDENTITY_SYMBOL),
        _ => SymbolType::from(
            foma::sigma::sigma_string(n, sigma).expect("arc symbol number resolves in sigma"),
        ),
    }
}

// [spec:hfst:def:foma-backend.backend-impl]
impl Backend for FomaTransducer {
    const TYPE: ImplementationType = ImplementationType::FOMA_TYPE;

    fn empty() -> Self {
        // fsm_empty_set returns Box<Fsm>; move the Fsm out of the box.
        FomaTransducer(*foma::structures::fsm_empty_set())
    }

    fn copy(&self) -> crate::error::Result<Self> {
        // fsm_copy borrows &mut Fsm (it refreshes the source's counts) and
        // returns a deep Box<Fsm> copy; clone into an owned mutable Fsm first.
        let mut src = self.0.clone();
        let copied = foma::structures::fsm_copy(&mut src);
        Ok(FomaTransducer(*copied))
    }

    // [spec:hfst:def:foma-backend.to-basic-fn]
    // [spec:hfst:sem:foma-backend.to-basic-fn]
    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        let mut net = HfstBasicTransducer::new();
        let sigma = self.0.sigma.as_deref();

        // Walk the line table in order, stopping at the sentinel row.
        for line in &self.0.states {
            if line.state_no == -1 {
                break;
            }
            let s = line.state_no as u32;
            // Ensure the state exists even if it has no arcs.
            net.add_state(s);
            // foma is unweighted -> final weight 0.0. foma's start state is
            // always state 0, matching HFST, so no start remapping is needed.
            if line.final_state == 1 {
                net.set_final_weight(s, &(0.0 as WeightType));
            }
            // An arc row has a real input symbol and target.
            if line.r#in != -1 && line.target != -1 {
                let isym = sym(line.r#in as i32, sigma);
                let osym = sym(line.out as i32, sigma);
                let tr = HfstBasicTransition::new_symbols(
                    line.target as u32,
                    isym,
                    osym,
                    0.0 as WeightType,
                    net.coder_mut(),
                );
                net.add_transition(s, &tr, true);
            }
        }

        // Every non-reserved sigma symbol joins the alphabet. Reserved numbers
        // 0/1/2 are represented by their HFST special strings, never added as
        // ordinary alphabet members.
        let mut node = self.0.sigma.as_deref();
        while let Some(n) = node {
            if n.number == -1 {
                break;
            }
            if n.number > foma::types::IDENTITY {
                if let Some(s) = n.symbol.as_deref() {
                    net.add_symbol_to_alphabet(&SymbolType::from(s));
                }
            }
            node = n.next.as_deref();
        }

        net.name = self.0.name.clone();
        Ok(net)
    }

    // [spec:hfst:def:foma-backend.from-basic-fn]
    // [spec:hfst:sem:foma-backend.from-basic-fn]
    fn from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        // foma's dynarray construction API interns each symbol string into a
        // sigma number: the special strings map to reserved numbers (0/1/2 via
        // the FOMA_RESERVED_SYMBOLS table), every other distinct symbol gets a
        // fresh number >= 3. HFST state 0 is the foma start state.
        let mut handle = foma::dynarray::fsm_construct_init(&net.name);
        foma::dynarray::fsm_construct_set_initial(&mut handle, 0);

        let coder = net.coder();
        for (s, transitions) in net.states_and_transitions().iter().enumerate() {
            let origin = s as i32;
            if net.is_final_state(s as u32) {
                foma::dynarray::fsm_construct_set_final(&mut handle, origin);
            }
            for tr in transitions.iter() {
                let isym = tr.get_input_symbol(coder);
                let osym = tr.get_output_symbol(coder);
                foma::dynarray::fsm_construct_add_arc(
                    &mut handle,
                    origin,
                    tr.get_target_state() as i32,
                    isym.as_str(),
                    osym.as_str(),
                );
            }
        }

        let fsm = foma::dynarray::fsm_construct_done(handle);
        Ok(FomaTransducer(*fsm))
    }

    fn get_alphabet(&self) -> StringSet {
        // The sigma's non-reserved symbols (numbers > IDENTITY).
        let mut out = StringSet::new();
        let mut node = self.0.sigma.as_deref();
        while let Some(n) = node {
            if n.number == -1 {
                break;
            }
            if n.number > foma::types::IDENTITY {
                if let Some(s) = n.symbol.as_deref() {
                    out.insert(SymbolType::from(s));
                }
            }
            node = n.next.as_deref();
        }
        out
    }

    fn is_cyclic(&self) -> bool {
        // fsm_topsort sets is_loop_free (1 acyclic, 0 cyclic) on the net it
        // returns; run it on a copy so this query stays non-destructive.
        let sorted = foma::topsort::fsm_topsort(Box::new(self.0.clone()));
        sorted.is_loop_free == 0
    }

    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        if self.0.sigma.is_none() {
            self.0.sigma = Some(foma::sigma::sigma_create());
        }
        foma::sigma::sigma_add(symbol, self.0.sigma.as_deref_mut().unwrap());
        Ok(())
    }

    fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        // Approximation for the seam: a cyclic transducer is infinitely
        // ambiguous on some input. Refined by foma-backend.lookup, which can
        // restrict cyclicity to the relevant input projection.
        Ok(self.is_cyclic())
    }

    fn write(&self, _os: &mut dyn std::io::Write, _hfst_format: bool) -> crate::error::Result<()> {
        // ported by foma-backend.io
        todo!("foma-backend.io: native .foma write")
    }

    fn extract_paths_cb(&self, _callback: &mut dyn ExtractStringsCb, _cycles: i32) {
        // ported by foma-backend.lookup
        todo!("foma-backend.lookup: path extraction")
    }

    fn extract_paths_fd_cb(
        &self,
        _callback: &mut dyn ExtractStringsCb,
        _cycles: i32,
        _filter_fd: bool,
    ) {
        // ported by foma-backend.lookup
        todo!("foma-backend.lookup: path extraction")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Reduce a basic transducer to a value that captures its recognized
    /// relation and alphabet: (state count, final states, alphabet, arcs).
    fn snapshot(
        net: &HfstBasicTransducer,
    ) -> (
        usize,
        BTreeSet<u32>,
        BTreeSet<String>,
        BTreeSet<(u32, String, String, u32)>,
    ) {
        let coder = net.coder();
        let n_states = (net.get_max_state() + 1) as usize;
        let mut finals = BTreeSet::new();
        let mut arcs = BTreeSet::new();
        for (s, transitions) in net.states_and_transitions().iter().enumerate() {
            let s = s as u32;
            if net.is_final_state(s) {
                finals.insert(s);
            }
            for tr in transitions.iter() {
                arcs.insert((
                    s,
                    tr.get_input_symbol(coder).to_string(),
                    tr.get_output_symbol(coder).to_string(),
                    tr.get_target_state(),
                ));
            }
        }
        let alphabet = net
            .get_alphabet()
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<String>>();
        (n_states, finals, alphabet, arcs)
    }

    /// Build the foma net for the relation a:b (state 0 -a:b-> state 1 final)
    /// via foma's construction API.
    fn build_ab() -> FomaTransducer {
        let mut handle = foma::dynarray::fsm_construct_init("ab");
        foma::dynarray::fsm_construct_set_initial(&mut handle, 0);
        foma::dynarray::fsm_construct_add_arc(&mut handle, 0, 1, "a", "b");
        foma::dynarray::fsm_construct_set_final(&mut handle, 1);
        FomaTransducer(*foma::dynarray::fsm_construct_done(handle))
    }

    // [spec:hfst:sem:foma-backend.to-basic-fn/test]
    // [spec:hfst:sem:foma-backend.from-basic-fn/test]
    #[test]
    fn round_trip_preserves_relation_and_alphabet() {
        let foma = build_ab();

        // foma -> basic
        let basic1 = foma.to_basic().expect("to_basic");
        // basic -> foma -> basic
        let foma2 = FomaTransducer::from_basic(&basic1).expect("from_basic");
        let basic2 = foma2.to_basic().expect("to_basic (round-trip)");

        let s1 = snapshot(&basic1);
        let s2 = snapshot(&basic2);

        // The round trip is stable across states, finals, alphabet and arcs.
        assert_eq!(s1, s2, "round-trip to_basic∘from_basic must be stable");

        // And the concrete shape is what we built.
        assert_eq!(s1.0, 2, "two states");
        assert_eq!(s1.1, BTreeSet::from([1u32]), "state 1 is final");
        assert!(s1.2.contains("a"), "alphabet has a");
        assert!(s1.2.contains("b"), "alphabet has b");
        assert!(
            s1.3.contains(&(0u32, "a".to_string(), "b".to_string(), 1u32)),
            "arc 0 -a:b-> 1 recognized"
        );
    }
}
