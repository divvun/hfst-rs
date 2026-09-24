//! Value types for lookup: symbol-pair tapes, weighted tapes and search steps.

use super::*;

// [spec:hfst:def:transducer.hfst-ol.symbol-pair]
#[derive(Clone, Copy)]
pub struct SymbolPair {
    pub input: SymbolNumber,
    pub output: SymbolNumber,
}

impl SymbolPair {
    // [spec:hfst:def:transducer.hfst-ol.symbol-pair.symbol-pair-fn]
    // [spec:hfst:sem:transducer.hfst-ol.symbol-pair.symbol-pair-fn]
    pub fn new() -> Self {
        SymbolPair {
            input: 0,
            output: 0,
        }
    }
    pub fn new_values(i: SymbolNumber, o: SymbolNumber) -> Self {
        SymbolPair {
            input: i,
            output: o,
        }
    }
}

impl Default for SymbolPair {
    fn default() -> Self {
        Self::new()
    }
}

// A vector that can be written to at any position, so that it
// adds new elements if the desired element isn't already present.
// [spec:hfst:def:transducer.hfst-ol.double-tape]
#[derive(Clone)]
pub struct DoubleTape {
    pub inner: Vec<SymbolPair>,
}

impl DoubleTape {
    pub fn new() -> Self {
        DoubleTape { inner: Vec::new() }
    }

    #[inline]
    pub fn write_pair(&mut self, pos: u32, input: SymbolNumber, out: SymbolNumber) {
        while pos as usize >= self.inner.len() {
            self.inner.push(SymbolPair::new());
        }
        self.inner[pos as usize] = SymbolPair::new_values(input, out);
    }

    pub fn write_vec(&mut self, pos: u32, vec: &[SymbolNumber]) {
        while pos as usize + vec.len() >= self.inner.len() {
            self.inner.push(SymbolPair::new());
        }
        for (i, &v) in vec.iter().enumerate() {
            self.inner[pos as usize + i] = SymbolPair::new_values(v, v);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.double-tape.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.double-tape.write-fn]
    // The C++ 'write(pos, pair<iterator,iterator>)' over a '[start, end)' slice.
    pub fn write_slice(&mut self, pos: u32, slice: &[SymbolNumber]) {
        let size = slice.len();
        while pos as usize + size >= self.inner.len() {
            self.inner.push(SymbolPair::new());
        }
        for (i, &v) in slice.iter().enumerate() {
            self.inner[pos as usize + i] = SymbolPair::new_values(v, v);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.double-tape.extract-slice-fn]
    // [spec:hfst:sem:transducer.hfst-ol.double-tape.extract-slice-fn]
    pub fn extract_slice(&self, mut start: u32, stop: u32) -> DoubleTape {
        let mut retval = DoubleTape::new();
        while start < stop {
            retval.inner.push(self.inner[start as usize]);
            start += 1;
        }
        retval
    }
}

impl Default for DoubleTape {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:transducer.hfst-ol.weighted-double-tape]
#[derive(Clone)]
pub struct WeightedDoubleTape {
    pub tape: DoubleTape,
    pub weight: Weight,
}

impl WeightedDoubleTape {
    // [spec:hfst:def:transducer.hfst-ol.weighted-double-tape.weighted-double-tape-fn]
    // [spec:hfst:sem:transducer.hfst-ol.weighted-double-tape.weighted-double-tape-fn]
    pub fn new(dt: DoubleTape, w: Weight) -> Self {
        WeightedDoubleTape {
            tape: dt,
            weight: w,
        }
    }
}

// [spec:hfst:def:transducer.hfst-ol.tape]
#[derive(Clone)]
pub struct Tape {
    pub inner: SymbolNumberVector,
}

impl Tape {
    pub fn new() -> Self {
        Tape { inner: Vec::new() }
    }

    // [spec:hfst:def:transducer.hfst-ol.tape.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.tape.write-fn]
    #[inline]
    pub fn write(&mut self, i: u32, s: SymbolNumber) {
        if self.inner.len() > i as usize {
            self.inner[i as usize] = s;
        } else {
            while self.inner.len() <= i as usize {
                self.inner.push(NO_SYMBOL_NUMBER);
            }
            self.inner[i as usize] = s;
        }
    }

    #[inline]
    pub fn at(&self, i: u32) -> SymbolNumber {
        self.inner[i as usize]
    }
}

impl Default for Tape {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:transducer.hfst-ol.string-weight-pair]
pub type StringWeightPair = (String, Weight);

// [spec:hfst:def:transducer.hfst-ol.s-transition]
pub struct STransition {
    pub index: TransitionTableIndex,
    pub symbol: SymbolNumber,
    pub weight: Weight,
}

impl STransition {
    pub fn new(i: TransitionTableIndex, s: SymbolNumber) -> Self {
        STransition {
            index: i,
            symbol: s,
            weight: 0.0,
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.s-transition.s-transition-fn]
    // [spec:hfst:sem:transducer.hfst-ol.s-transition.s-transition-fn]
    pub fn new_weighted(i: TransitionTableIndex, s: SymbolNumber, w: Weight) -> Self {
        STransition {
            index: i,
            symbol: s,
            weight: w,
        }
    }
}

// [spec:hfst:def:transducer.hfst-ol.n-byte-utf8-fn]
// [spec:hfst:sem:transducer.hfst-ol.n-byte-utf8-fn]
// [spec:hfst:def:ospell.hfst-ol.n-byte-utf8-fn]
// [spec:hfst:sem:ospell.hfst-ol.n-byte-utf8-fn]
// (declared in transducer.h, defined in ospell.cc — one function, two ids)
/// How many bytes to peel off the tape as one UTF-8 character (for
/// representing it as OTHER), judged from the lead byte; `None` on a
/// continuation byte. Like the C original, invalid `11111xxx` lead bytes
/// are leniently treated as 4-byte sequences.
pub fn utf8_sequence_length(lead: u8) -> Option<usize> {
    match lead.leading_ones() {
        0 => Some(1),
        2 => Some(2),
        3 => Some(3),
        n if n >= 4 => Some(4),
        _ => None,
    }
}

// 'void increment_mutator(void)' on 'TreeNode' (declared in transducer.h:1503)
// is only declared, never defined anywhere in the codebase — effectively dead.
// A faithful port is an empty stub; there is no behavior to replicate. 'TreeNode'
// itself lives in the 'ospell' module; this inherent impl is a same-crate split.
impl crate::ospell::TreeNode {
    // [spec:hfst:def:transducer.hfst-ol.tree-node.increment-mutator-fn]
    // [spec:hfst:sem:transducer.hfst-ol.tree-node.increment-mutator-fn]
    pub fn increment_mutator(&mut self) {}
}
