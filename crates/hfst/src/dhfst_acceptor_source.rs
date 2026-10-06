//! The source of a DHFST acceptor ([`crate::dhfst_acceptor`]): a weighted
//! optimized-lookup transducer read exactly as divvunspell's suggestion
//! search reads a lexicon from its file, through the HFST reader's
//! `free_arcs`, `has_transitions`, `transitions`, `is_final`, `final_weight`
//! and `distance_to_final`. Authored greenfield against
//! `docs/spec/port/back-ends/dhfst/dhfst.md`; the writer is
//! [`crate::dhfst_acceptor_writer`].
//!
//! States are optimized-lookup addresses: an index-table position, or the
//! transition-table start plus the position of the state's head record. The
//! start state is address 0.

use crate::dhfst_distance::Distances;
use crate::dhfst_source::OlView;
use crate::hfst_transducer::HfstTransducer;
use crate::transducer::{Transducer, WeightedTables};

/// One transition as the search reads it: its output, target and weight,
/// each `None` where the table says "none".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    /// output symbol
    pub output: Option<u16>,
    /// target address
    pub target: Option<u32>,
    /// weight
    pub weight: Option<f32>,
}

/// A weighted optimized-lookup transducer, read as the search reads it, and
/// the lookahead distance of each of its states.
pub struct AcceptorSource<'a> {
    ol: OlView<'a>,
    distances: Distances,
}

impl<'a> AcceptorSource<'a> {
    /// Open `t` for reading, and compute its distances. Refuses a symbol
    /// table divvunspell would refuse to load.
    // [spec:hfst:def:dhfst.acceptor-source]
    pub fn new(
        t: &'a HfstTransducer<Transducer<WeightedTables>>,
    ) -> crate::error::Result<AcceptorSource<'a>> {
        let ol = OlView::new(&t.fst)?;
        let distances = Distances::compute(&ol);
        Ok(AcceptorSource { ol, distances })
    }

    /// The number of symbols the header declares.
    pub fn symbol_count(&self) -> usize {
        self.ol.symbols.len()
    }

    /// Symbol names, `@_EPSILON_SYMBOL_@` first.
    pub fn symbol_names(&self) -> Vec<String> {
        self.ol.symbols.iter().map(|s| s.to_string()).collect()
    }

    /// Whether `symbol` is free: epsilon or a flag diacritic.
    pub fn is_free(&self, symbol: u16) -> bool {
        symbol == 0 || self.ol.is_flag(symbol)
    }

    /// Whether the state at `state` is final.
    pub fn is_final(&self, state: u32) -> bool {
        self.ol.is_final(state)
    }

    /// The final weight the tables hold for `state`, final or not.
    pub fn final_weight(&self, state: u32) -> Option<f32> {
        self.ol.final_weight(state)
    }

    /// The least weight of any path from `state` to a final state, final
    /// weight included; 0 when the distances say nothing.
    pub fn distance(&self, state: u32) -> f32 {
        self.distances.get(state)
    }

    /// Whether `state` has epsilon or flag diacritic arcs, as the search
    /// asks it: the cursor one past the state reads epsilon, or for a
    /// transition-table state a free symbol.
    pub fn has_free_arcs(&self, state: u32) -> bool {
        let i = state.wrapping_add(1);
        if i >= crate::dhfst_source::TARGET_TABLE {
            self.ol
                .trans_input(i - crate::dhfst_source::TARGET_TABLE)
                .is_some_and(|s| self.is_free(s))
        } else {
            self.ol.index_input(i) == Some(0)
        }
    }

    /// The epsilon and flag diacritic arcs of `state`, each as its input
    /// symbol and transition, in stored order.
    // [spec:hfst:sem:dhfst.acceptor-source]
    pub fn free_arcs(&self, state: u32) -> impl Iterator<Item = (u16, Transition)> + '_ {
        let mut next = if self.has_free_arcs(state) {
            self.ol.next(state, 0)
        } else {
            None
        };
        std::iter::from_fn(move || {
            let at = next?;
            let Some(input) = self.ol.trans_input(at).filter(|s| self.is_free(*s)) else {
                next = None;
                return None;
            };
            next = Some(at + 1);
            Some((input, self.transition(at)))
        })
    }

    /// Whether `state` has arcs on `symbol`, as the search asks it.
    pub fn has_transitions(&self, state: u32, symbol: u16) -> bool {
        self.ol.has_transitions(state.wrapping_add(1), symbol)
    }

    /// The transitions of `state` on `symbol`, in stored order, as the
    /// search reads them once [`Self::has_transitions`] has said yes.
    // [spec:hfst:sem:dhfst.acceptor-source]
    pub fn transitions(&self, state: u32, symbol: u16) -> impl Iterator<Item = Transition> + '_ {
        let mut next = self.ol.next(state, symbol);
        std::iter::from_fn(move || {
            let at = next?;
            if self.ol.trans_input(at) != Some(symbol) {
                next = None;
                return None;
            }
            next = Some(at + 1);
            Some(self.transition(at))
        })
    }

    /// Transition-table record `at`.
    fn transition(&self, at: u32) -> Transition {
        Transition {
            output: self.ol.trans_output(at),
            target: self.ol.trans_target(at),
            weight: self.ol.trans_weight(at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dhfst_acceptor_writer::tests::{LEXICON, lexicon};

    /// The index of the symbol named `name`.
    fn symbol(source: &AcceptorSource<'_>, name: &str) -> u16 {
        source
            .symbol_names()
            .iter()
            .position(|s| s == name)
            .expect("the symbol is in the alphabet") as u16
    }

    // [spec:hfst:def:dhfst.acceptor-source/test]
    // [spec:hfst:sem:dhfst.acceptor-source/test]
    #[test]
    fn reads_the_arcs_the_search_reads() {
        let t = lexicon(LEXICON);
        let source = AcceptorSource::new(&t).expect("the lexicon opens");
        assert!(source.is_free(0) && source.is_free(symbol(&source, "@P.CASE.NOM@")));
        assert!(!source.is_free(symbol(&source, "a")));
        // The start state's free arcs: the epsilon arc, then the flag.
        let free: Vec<u16> = source.free_arcs(0).map(|(input, _)| input).collect();
        assert_eq!(free, vec![0, symbol(&source, "@P.CASE.NOM@")]);
        assert!(
            source
                .free_arcs(0)
                .all(|(input, t)| t.output == Some(input))
        );
        let a = symbol(&source, "a");
        assert!(source.has_transitions(0, a));
        let weights: Vec<Option<f32>> = source.transitions(0, a).map(|t| t.weight).collect();
        assert_eq!(weights, vec![Some(0.5), Some(1.0)]);
        assert!(!source.has_transitions(0, symbol(&source, "g")));
        assert!(!source.is_final(0));
        assert!(source.distance(0) > 0.0);
    }

    // [spec:hfst:sem:dhfst.acceptor-source/test]
    #[test]
    fn refuses_a_flag_operator_divvunspell_does_not_know() {
        let t = lexicon("0\t1\t@Q.CASE.NOM@\t@Q.CASE.NOM@\t0\n1\t0\n");
        match AcceptorSource::new(&t) {
            Ok(_) => panic!("an alphabet divvunspell refuses was opened"),
            Err(e) => assert!(e.to_string().contains("operator"), "{e}"),
        }
    }
}
