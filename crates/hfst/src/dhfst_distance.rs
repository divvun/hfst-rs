//! The lookahead distance of every state of a weighted optimized-lookup
//! transducer: the least weight of any path from the state to a final state,
//! final weight included, `+inf` when there is none. Authored greenfield
//! against `docs/spec/port/back-ends/dhfst/dhfst.md`; the algorithm is
//! divvunspell's `src/transducer/heuristic.rs`, the values are its values bit
//! for bit, and the suggestion search adds them to a path's weight to order its
//! queue. [`crate::dhfst_acceptor_writer`] stores them in an acceptor's `DIST`
//! section.
//!
//! The distances are a shortest distance in the reverse graph from every final
//! state, by Dijkstra. Every table position is a node: index-table entry `i`
//! is node `i`, and the transition-table state whose head record is `p` is
//! node `index size + p`. The edges are read off the tables as divvunspell
//! reads them: each tagged index entry is the row of the state `entry -
//! symbol - 1`, the epsilon row taking the flag diacritic arcs that follow
//! it, and a transition-table state found through an arc takes its whole
//! block. Negative weights are floored at zero and their total taken back off
//! every distance at the end; with more than 0.01 of them, or a NaN, every
//! distance is 0. So is every distance when fewer than one reachable node in
//! 1000 has a distance above 0.001, which is what a weight-pushed lexicon
//! gives.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::dhfst_source::{OlView, TARGET_TABLE};

/// The most negative weight, in total, that is taken as rounding residue.
const MAX_SLACK: f32 = 0.01;
/// The distance at or below which a node says nothing.
const FLAT_TOLERANCE: f32 = 1e-3;
/// At least one reachable node in this many must say something.
const INFORMATIVE_IN: usize = 1000;

/// A weight ordered by [`f32::total_cmp`], as divvunspell orders the queue.
#[derive(Clone, Copy, Debug)]
struct Ordered(f32);

impl PartialEq for Ordered {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for Ordered {}

impl PartialOrd for Ordered {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ordered {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Which records belong to the run of arcs being scanned.
#[derive(Clone, Copy)]
enum Run {
    /// consecutive records with this input symbol
    Symbol(u16),
    /// consecutive epsilon or flag diacritic records
    Free,
    /// every record up to the next head record
    Block,
}

/// The distance of every node; empty when every distance is 0.
// [spec:hfst:def:dhfst.acceptor-distance]
pub struct Distances {
    index: Vec<f32>,
    trans: Vec<f32>,
}

impl Distances {
    /// Every distance 0.
    fn flat() -> Distances {
        Distances {
            index: Vec::new(),
            trans: Vec::new(),
        }
    }

    /// The distance of the state at `address`, 0 for an address outside the
    /// tables or when every distance is 0.
    pub fn get(&self, address: u32) -> f32 {
        let slot = if address >= TARGET_TABLE {
            self.trans.get((address - TARGET_TABLE) as usize)
        } else {
            self.index.get(address as usize)
        };
        slot.copied().unwrap_or(0.0)
    }

    /// Whether every distance is 0.
    pub fn is_flat(&self) -> bool {
        self.index.is_empty() && self.trans.is_empty()
    }

    /// Compute the distances of every node of `ol`.
    // [spec:hfst:sem:dhfst.acceptor-distance]
    pub(crate) fn compute(ol: &OlView<'_>) -> Distances {
        let n_index = ol.index_size as usize;
        let n = n_index + ol.target_size as usize;
        if n == 0 || n > u32::MAX as usize {
            return Distances::flat();
        }
        let mut slack = 0.0f32;
        let mut poisoned = false;
        let mut dist = seeds(ol, &mut slack, &mut poisoned);

        // The reverse graph as compressed rows, from two identical walks: the
        // first counts the edges into each node, the second fills them in.
        let mut offsets = vec![0u32; n + 1];
        let mut edge_count = 0usize;
        walk_edges(ol, &mut |_, to, w| {
            tally(w, &mut slack, &mut poisoned);
            offsets[to as usize + 1] += 1;
            edge_count += 1;
        });
        if poisoned || slack > MAX_SLACK || edge_count > u32::MAX as usize {
            return Distances::flat();
        }
        for i in 0..n {
            offsets[i + 1] += offsets[i];
        }
        // Filling moves `offsets[to]` past each edge written, which leaves it
        // at the end of the node's edges; they start at `offsets[to - 1]`.
        let mut edges: Vec<(u32, f32)> = vec![(0, 0.0); edge_count];
        walk_edges(ol, &mut |from, to, w| {
            let slot = &mut offsets[to as usize];
            edges[*slot as usize] = (from, clamp(w));
            *slot += 1;
        });

        settle(&mut dist, &offsets, &edges);
        if slack > 0.0 {
            for d in dist.iter_mut().filter(|d| d.is_finite()) {
                *d -= slack;
            }
        }
        if !informative(&dist, &offsets) {
            return Distances::flat();
        }
        let trans = dist.split_off(n_index);
        Distances { index: dist, trans }
    }
}

/// Every node at `+inf`, but the final states at their final weights,
/// clamped.
fn seeds(ol: &OlView<'_>, slack: &mut f32, poisoned: &mut bool) -> Vec<f32> {
    let n_index = ol.index_size as usize;
    let mut dist = vec![f32::INFINITY; n_index + ol.target_size as usize];
    for (i, slot) in dist.iter_mut().enumerate().take(n_index) {
        if ol.index_is_final(i as u32)
            && let Some(w) = ol.index_final_weight(i as u32)
        {
            tally(w, slack, poisoned);
            *slot = clamp(w);
        }
    }
    for (p, slot) in dist.iter_mut().skip(n_index).enumerate() {
        if ol.trans_is_final(p as u32)
            && let Some(w) = ol.trans_weight(p as u32)
        {
            tally(w, slack, poisoned);
            *slot = clamp(w);
        }
    }
    dist
}

/// Dijkstra from every node with a finite distance, over the reverse edges:
/// the edges into node `i` are `edges[offsets[i - 1]..offsets[i]]`.
fn settle(dist: &mut [f32], offsets: &[u32], edges: &[(u32, f32)]) {
    let mut queue: BinaryHeap<Reverse<(Ordered, u32)>> = BinaryHeap::new();
    for (id, d) in dist.iter().enumerate() {
        if *d != f32::INFINITY {
            queue.push(Reverse((Ordered(*d), id as u32)));
        }
    }
    while let Some(Reverse((Ordered(d), node))) = queue.pop() {
        if d.total_cmp(&dist[node as usize]).is_gt() {
            continue;
        }
        let end = offsets[node as usize] as usize;
        let start = match node {
            0 => 0,
            _ => offsets[node as usize - 1] as usize,
        };
        for &(predecessor, weight) in &edges[start..end] {
            let relaxed = d + weight;
            if relaxed.total_cmp(&dist[predecessor as usize]).is_lt() {
                dist[predecessor as usize] = relaxed;
                queue.push(Reverse((Ordered(relaxed), predecessor)));
            }
        }
    }
}

/// Whether at least one reachable node in [`INFORMATIVE_IN`] has a distance
/// above [`FLAT_TOLERANCE`]. A node is reachable when some edge leads into
/// it; node 0 always is.
fn informative(dist: &[f32], offsets: &[u32]) -> bool {
    let mut reachable = 0usize;
    let mut informative = 0usize;
    for (id, d) in dist.iter().enumerate() {
        let start = if id == 0 { 0 } else { offsets[id - 1] };
        if id != 0 && offsets[id] == start {
            continue;
        }
        reachable += 1;
        if *d > FLAT_TOLERANCE {
            informative += 1;
        }
    }
    informative * INFORMATIVE_IN >= reachable
}

/// A weight floored at zero, as Dijkstra needs.
fn clamp(w: f32) -> f32 {
    if w > 0.0 { w } else { 0.0 }
}

/// Count what [`clamp`] takes from a negative weight into `slack`; a NaN
/// sets `poisoned`.
fn tally(w: f32, slack: &mut f32, poisoned: &mut bool) {
    if w.is_nan() {
        *poisoned = true;
    } else if w < 0.0 {
        *slack -= w;
    }
}

/// Hand `emit` every edge as `(from, to, weight)` between nodes, in the same
/// order every time.
fn walk_edges(ol: &OlView<'_>, emit: &mut impl FnMut(u32, u32, f32)) {
    let n_index = ol.index_size;
    let mut walk = EdgeWalk {
        ol,
        n_index,
        seen: vec![false; ol.target_size as usize],
        pending: Vec::new(),
    };
    for entry in 0..n_index {
        let (Some(symbol), Some(target)) = (ol.index_input(entry), ol.index_target(entry)) else {
            continue;
        };
        if target < TARGET_TABLE {
            continue;
        }
        let Some(source) = entry.checked_sub(symbol as u32 + 1) else {
            continue;
        };
        let run = match symbol {
            0 => Run::Free,
            s => Run::Symbol(s),
        };
        walk.scan(target - TARGET_TABLE, run, source, emit);
    }
    while let Some(head) = walk.pending.pop() {
        walk.scan(head.saturating_add(1), Run::Block, n_index + head, emit);
    }
}

/// The transition-table states an edge walk has found, and those it has
/// still to scan.
struct EdgeWalk<'v, 'a> {
    ol: &'v OlView<'a>,
    n_index: u32,
    seen: Vec<bool>,
    pending: Vec<u32>,
}

impl EdgeWalk<'_, '_> {
    /// Emit the arcs of one run of transition records from `start`, as
    /// edges from node `source`.
    fn scan(&mut self, start: u32, run: Run, source: u32, emit: &mut impl FnMut(u32, u32, f32)) {
        let ol = self.ol;
        let mut record = start;
        while record < ol.target_size {
            let Some(symbol) = ol.trans_input(record) else {
                break;
            };
            let in_run = match run {
                Run::Symbol(wanted) => symbol == wanted,
                Run::Free => symbol == 0 || ol.is_flag(symbol),
                Run::Block => true,
            };
            if !in_run {
                break;
            }
            if let (Some(target), Some(weight)) = (ol.trans_target(record), ol.trans_weight(record))
            {
                self.edge(source, target, weight, emit);
            }
            record += 1;
        }
    }

    /// Emit the edge of one arc to `target`, queueing a transition-table
    /// state the first time an arc leads to it.
    fn edge(
        &mut self,
        source: u32,
        target: u32,
        weight: f32,
        emit: &mut impl FnMut(u32, u32, f32),
    ) {
        if target >= TARGET_TABLE {
            let head = target - TARGET_TABLE;
            if head < self.ol.target_size {
                emit(source, self.n_index + head, weight);
                if !self.seen[head as usize] {
                    self.seen[head as usize] = true;
                    self.pending.push(head);
                }
            }
        } else if target < self.n_index {
            emit(source, target, weight);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::convert_transducer_format::ConversionFunctions;
    use crate::hfst_basic_transducer::HfstBasicTransducer;
    use crate::transducer::{Transducer, WeightedTables};

    /// Weighted optimized lookup from AT&T text.
    pub(crate) fn olw(att: &str) -> Transducer<WeightedTables> {
        let mut lines = 0u32;
        let net =
            HfstBasicTransducer::read_in_att_format(&mut att.as_bytes(), "@0@", &mut lines, false)
                .expect("the AT&T text parses");
        ConversionFunctions::hfst_basic_transducer_to_hfst_ol(&net, true, "", None)
            .expect("the transducer converts to optimized lookup")
    }

    /// The least weight from each state of a small automaton to a final
    /// state, by relaxing every arc until nothing changes.
    fn by_relaxation(arcs: &[(usize, usize, f32)], finals: &[(usize, f32)], n: usize) -> Vec<f32> {
        let mut dist = vec![f32::INFINITY; n];
        for (q, w) in finals {
            dist[*q] = *w;
        }
        for _ in 0..n {
            for (from, to, w) in arcs {
                dist[*from] = dist[*from].min(dist[*to] + w);
            }
        }
        dist
    }

    // [spec:hfst:def:dhfst.acceptor-distance/test]
    // [spec:hfst:sem:dhfst.acceptor-distance/test]
    #[test]
    fn distances_are_shortest_paths_to_a_final_state() {
        // 0 -a-> 1 -b-> 2 (final 1.5), 0 -c-> 3 (final 4), 1 -ε-> 3, and a
        // dead end 4 that no final state follows.
        let t = olw(
            "0\t1\ta\ta\t0.5\n1\t2\tb\tb\t1\n0\t3\tc\tc\t0.25\n1\t3\t@0@\t@0@\t2\n0\t4\td\td\t1\n2\t1.5\n3\t4\n",
        );
        let ol = OlView::new(&t).expect("the view opens");
        let d = Distances::compute(&ol);
        assert!(!d.is_flat());
        let want = by_relaxation(
            &[
                (0, 1, 0.5),
                (1, 2, 1.0),
                (0, 3, 0.25),
                (1, 3, 2.0),
                (0, 4, 1.0),
            ],
            &[(2, 1.5), (3, 4.0)],
            5,
        );
        // State 0 is the start; the others are found by their arcs.
        assert_eq!(d.get(0), want[0]);
        let mut seen: Vec<f32> = Vec::new();
        for input in 0..ol.symbols.len() as u16 {
            ol.for_each_arc(0, input, |_, target, _| seen.push(d.get(target)));
        }
        seen.sort_by(f32::total_cmp);
        let mut expected = vec![want[1], want[3], want[4]];
        expected.sort_by(f32::total_cmp);
        assert_eq!(seen, expected);
        assert_eq!(want[4], f32::INFINITY);
        assert_eq!(d.get(u32::MAX), 0.0);
    }

    // [spec:hfst:sem:dhfst.acceptor-distance/test]
    #[test]
    fn flat_and_untrustworthy_weights_give_no_distances() {
        // Weight pushed: every state finishes for nothing.
        let pushed = olw("0\t1\ta\ta\t0\n0\t1\tb\tb\t1\n1\t0\n");
        assert!(Distances::compute(&OlView::new(&pushed).expect("view")).is_flat());
        // More negative weight than rounding residue.
        let negative = olw("0\t1\ta\ta\t-1\n1\t2\tb\tb\t3\n2\t2\n");
        assert!(Distances::compute(&OlView::new(&negative).expect("view")).is_flat());
        // Rounding residue is floored and taken back off.
        let residue = olw("0\t1\ta\ta\t-0.000001\n1\t2\tb\tb\t3\n2\t2\n");
        let d = Distances::compute(&OlView::new(&residue).expect("view"));
        assert_eq!(d.get(0), 5.0 - 0.000001);
    }
}
