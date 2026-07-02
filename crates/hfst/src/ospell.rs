//! Port of 'libhfst/src/implementations/optimized-lookup/ospell.cc' (namespace
//! 'hfst_ol') — the two-transducer spellchecker ('Speller') plus the
//! 'TreeNode'/'InputString'/'AlphabetTranslationException'/priority-queue types
//! it uses (declared in 'transducer.h', defined here; co-located in this module
//! since they form one coherent ospell unit). 'nByte_utf8''s body lives in
//! 'transducer.rs' (it is declared in 'transducer.h').
//!
//! The 'mutator'/'lexicon' 'Transducer*' raw pointers are non-owning in the
//! C++; they become shared '&'a Transducer' references here, since every method
//! the speller calls on them reads only.

use std::collections::{BTreeMap, VecDeque};

use crate::hfst_flag_diacritics::FdState;
use crate::transducer::{
    Encoder, NO_SYMBOL_NUMBER, STransition, StringWeightPair, SymbolNumber, SymbolNumberVector,
    SymbolTable, Transducer, TransitionTableIndex, Weight, nByte_utf8,
};

// [spec:hfst:def:transducer.hfst-ol.string-weight-comparison]
#[derive(Clone)]
pub struct StringWeightComparison {
    reverse: bool,
}

impl StringWeightComparison {
    // [spec:hfst:def:transducer.hfst-ol.string-weight-comparison.string-weight-comparison-fn]
    // [spec:hfst:sem:transducer.hfst-ol.string-weight-comparison.string-weight-comparison-fn]
    pub fn new(reverse_result: bool) -> Self {
        StringWeightComparison {
            reverse: reverse_result,
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.string-weight-comparison.operator-fn]
    // [spec:hfst:sem:transducer.hfst-ol.string-weight-comparison.operator-fn]
    // return true when we want rhs to appear before lhs
    pub fn operator_call(&self, lhs: &StringWeightPair, rhs: &StringWeightPair) -> bool {
        if self.reverse {
            lhs.1 < rhs.1
        } else {
            lhs.1 > rhs.1
        }
    }
}

/// 'std::priority_queue<StringWeightPair, vector, StringWeightComparison>' — the
/// shared backing of CorrectionQueue / AnalysisQueue / HyphenationQueue. 'top'
/// is the greatest element under the comparator (a < b iff 'comp(a, b)'), i.e.
/// the smallest weight with the default (non-reversed) comparator.
#[derive(Clone)]
pub struct StringWeightPriorityQueue {
    data: Vec<StringWeightPair>,
    comp: StringWeightComparison,
}

impl StringWeightPriorityQueue {
    pub fn new() -> Self {
        StringWeightPriorityQueue {
            data: Vec::new(),
            comp: StringWeightComparison::new(false),
        }
    }

    pub fn push(&mut self, x: StringWeightPair) {
        self.data.push(x);
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn empty(&self) -> bool {
        self.data.is_empty()
    }

    fn top_index(&self) -> usize {
        let mut best = 0;
        for i in 1..self.data.len() {
            // best < data[i]  <=>  comp(best, data[i])
            if self.comp.operator_call(&self.data[best], &self.data[i]) {
                best = i;
            }
        }
        best
    }

    pub fn top(&self) -> &StringWeightPair {
        &self.data[self.top_index()]
    }

    pub fn pop(&mut self) {
        if self.data.is_empty() {
            return;
        }
        let best = self.top_index();
        self.data.remove(best);
    }
}

impl Default for StringWeightPriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:transducer.hfst-ol.correction-queue]
pub type CorrectionQueue = StringWeightPriorityQueue;
// [spec:hfst:def:transducer.hfst-ol.analysis-queue]
pub type AnalysisQueue = StringWeightPriorityQueue;
// [spec:hfst:def:transducer.hfst-ol.hyphenation-queue]
pub type HyphenationQueue = StringWeightPriorityQueue;

// [spec:hfst:def:transducer.hfst-ol.tree-node]
#[derive(Clone)]
pub struct TreeNode {
    pub string: SymbolNumberVector,
    pub input_state: u32,
    pub mutator_state: TransitionTableIndex,
    pub lexicon_state: TransitionTableIndex,
    pub flag_state: FdState<SymbolNumber>,
    pub weight: Weight,
}

impl TreeNode {
    // [spec:hfst:def:transducer.hfst-ol.tree-node.tree-node-fn]
    // [spec:hfst:sem:transducer.hfst-ol.tree-node.tree-node-fn]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        prev_string: SymbolNumberVector,
        i: u32,
        mutator: TransitionTableIndex,
        lexicon: TransitionTableIndex,
        state: FdState<SymbolNumber>,
        w: Weight,
    ) -> Self {
        TreeNode {
            string: prev_string,
            input_state: i,
            mutator_state: mutator,
            lexicon_state: lexicon,
            flag_state: state,
            weight: w,
        }
    }

    // starting state node
    pub fn new_start(start_state: FdState<SymbolNumber>) -> Self {
        TreeNode {
            string: SymbolNumberVector::new(),
            input_state: 0,
            mutator_state: 0,
            lexicon_state: 0,
            flag_state: start_state,
            weight: 0.0,
        }
    }

    // [spec:hfst:def:ospell.hfst-ol.tree-node.update-lexicon-fn]
    // [spec:hfst:sem:ospell.hfst-ol.tree-node.update-lexicon-fn]
    // [spec:hfst:def:transducer.hfst-ol.tree-node.update-lexicon-fn]
    // [spec:hfst:sem:transducer.hfst-ol.tree-node.update-lexicon-fn]
    pub fn update_lexicon(
        &self,
        next_symbol: SymbolNumber,
        next_lexicon: TransitionTableIndex,
        weight: Weight,
    ) -> TreeNode {
        let mut str = self.string.clone();
        str.push(next_symbol);
        TreeNode::new(
            str,
            self.input_state,
            self.mutator_state,
            next_lexicon,
            self.flag_state.clone(),
            self.weight + weight,
        )
    }

    // [spec:hfst:def:ospell.hfst-ol.tree-node.update-mutator-fn]
    // [spec:hfst:sem:ospell.hfst-ol.tree-node.update-mutator-fn]
    // [spec:hfst:def:transducer.hfst-ol.tree-node.update-mutator-fn]
    // [spec:hfst:sem:transducer.hfst-ol.tree-node.update-mutator-fn]
    pub fn update_mutator(
        &self,
        next_symbol: SymbolNumber,
        next_mutator: TransitionTableIndex,
        weight: Weight,
    ) -> TreeNode {
        let mut str = self.string.clone();
        str.push(next_symbol);
        TreeNode::new(
            str,
            self.input_state,
            next_mutator,
            self.lexicon_state,
            self.flag_state.clone(),
            self.weight + weight,
        )
    }

    // [spec:hfst:def:ospell.hfst-ol.tree-node.update-fn]
    // [spec:hfst:sem:ospell.hfst-ol.tree-node.update-fn]
    // [spec:hfst:def:transducer.hfst-ol.tree-node.update-fn]
    // [spec:hfst:sem:transducer.hfst-ol.tree-node.update-fn]
    #[allow(clippy::too_many_arguments)]
    pub fn update_input(
        &self,
        next_symbol: SymbolNumber,
        next_input: u32,
        next_mutator: TransitionTableIndex,
        next_lexicon: TransitionTableIndex,
        weight: Weight,
    ) -> TreeNode {
        let mut str = self.string.clone();
        str.push(next_symbol);
        TreeNode::new(
            str,
            next_input,
            next_mutator,
            next_lexicon,
            self.flag_state.clone(),
            self.weight + weight,
        )
    }

    pub fn update(
        &self,
        next_symbol: SymbolNumber,
        next_mutator: TransitionTableIndex,
        next_lexicon: TransitionTableIndex,
        weight: Weight,
    ) -> TreeNode {
        let mut str = self.string.clone();
        str.push(next_symbol);
        TreeNode::new(
            str,
            self.input_state,
            next_mutator,
            next_lexicon,
            self.flag_state.clone(),
            self.weight + weight,
        )
    }
}

// [spec:hfst:def:transducer.hfst-ol.tree-node-queue]
pub type TreeNodeQueue = VecDeque<TreeNode>;

// [spec:hfst:def:transducer.hfst-ol.input-string]
pub struct InputString {
    s: SymbolNumberVector,
}

impl InputString {
    // [spec:hfst:def:transducer.hfst-ol.input-string.input-string-fn]
    // [spec:hfst:sem:transducer.hfst-ol.input-string.input-string-fn]
    pub fn new() -> Self {
        InputString {
            s: SymbolNumberVector::new(),
        }
    }

    // [spec:hfst:def:ospell.hfst-ol.input-string.initialize-fn]
    // [spec:hfst:sem:ospell.hfst-ol.input-string.initialize-fn]
    // [spec:hfst:def:transducer.hfst-ol.input-string.initialize-fn]
    // [spec:hfst:sem:transducer.hfst-ol.input-string.initialize-fn]
    pub fn initialize(&mut self, encoder: &Encoder, input: &str, other: SymbolNumber) -> bool {
        // Initialize the symbol vector to the tokenization given by encoder. On
        // tokenization failure, valid utf-8 characters are tokenized as "other"
        // and tokenization is reattempted from there; the empty string is an
        // empty vector with no end marker.
        self.s.clear();
        let mut buf: Vec<u8> = input.as_bytes().to_vec();
        buf.push(0);
        let mut p: usize = 0;

        while buf[p] != 0 {
            let oldp = p;
            match encoder.find_key(&buf, &mut p) {
                None => {
                    // no tokenization from alphabet
                    let n = nByte_utf8(buf[oldp]);
                    if n == 0 {
                        return false; // can't parse utf-8 character, admit failure
                    }
                    if other == NO_SYMBOL_NUMBER {
                        return false; // if we don't have an "other" symbol
                    }
                    p = oldp + n as usize;
                    self.s.push(other);
                }
                Some(k) => {
                    self.s.push(k);
                }
            }
        }
        true
    }

    // [spec:hfst:def:transducer.hfst-ol.input-string.len-fn]
    // [spec:hfst:sem:transducer.hfst-ol.input-string.len-fn]
    pub fn len(&self) -> u32 {
        self.s.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.s.is_empty()
    }

    // [spec:hfst:def:transducer.hfst-ol.input-string.operator-fn]
    // [spec:hfst:sem:transducer.hfst-ol.input-string.operator-fn]
    pub fn at(&self, i: u32) -> SymbolNumber {
        self.s[i as usize]
    }
}

impl Default for InputString {
    fn default() -> Self {
        Self::new()
    }
}

// The ospell `AlphabetTranslationException` (which carried the first
// untranslatable symbol) is now `crate::error::ErrorKind::AlphabetTranslation`,
// with the symbol carried in the `Error` message.
// [spec:hfst:def:transducer.hfst-ol.alphabet-translation-exception]

/// A spellchecker, constructed from two optimized-lookup transducer instances.
/// An alphabet translator is built at construction time.
// [spec:hfst:def:transducer.hfst-ol.speller]
pub struct Speller<'a> {
    pub mutator: &'a Transducer,
    pub lexicon: &'a Transducer,
    pub input: InputString,
    pub queue: TreeNodeQueue,
    pub alphabet_translator: SymbolNumberVector,
    pub symbol_table: SymbolTable,
}

impl<'a> Speller<'a> {
    // [spec:hfst:def:transducer.hfst-ol.speller.speller-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.speller-fn]
    pub fn new(
        mutator_ptr: &'a Transducer,
        lexicon_ptr: &'a Transducer,
    ) -> crate::error::Result<Self> {
        let symbol_table = lexicon_ptr.get_symbol_table().clone();
        let mut speller = Speller {
            mutator: mutator_ptr,
            lexicon: lexicon_ptr,
            input: InputString::new(),
            queue: TreeNodeQueue::new(),
            alphabet_translator: SymbolNumberVector::new(),
            symbol_table,
        };
        speller.build_alphabet_translator()?;
        Ok(speller)
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.init-input-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.init-input-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.init-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.init-input-fn]
    pub fn init_input(&mut self, str: &str, encoder: &Encoder, other: SymbolNumber) -> bool {
        self.input.initialize(encoder, str, other)
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.build-alphabet-translator-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.build-alphabet-translator-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.build-alphabet-translator-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.build-alphabet-translator-fn]
    pub fn build_alphabet_translator(&mut self) -> crate::error::Result<()> {
        let mutator = self.mutator;
        let lexicon = self.lexicon;
        let from = mutator.get_alphabet();
        let to = lexicon.get_alphabet();
        let from_keys = from.get_symbol_table();
        let to_symbols = to.build_string_symbol_map();
        self.alphabet_translator.push(0); // zeroth element is always epsilon
        for i in 1..from_keys.len() {
            let i_sym = i as SymbolNumber;
            if from.is_flag_diacritic(i_sym) || i_sym == from.get_unknown_symbol() {
                // if it's a flag or the OTHER symbol
                self.alphabet_translator.push(NO_SYMBOL_NUMBER);
                continue; // no translation
            }
            if !to_symbols.contains_key(&from_keys[i]) {
                let name = from_keys[i].clone();
                if name != "" {
                    crate::bail!(AlphabetTranslation, from_keys[i].clone())
                }
            }
            // translator at i points to lexicon's symbol for mutator's string
            // for mutator's symbol number i
            self.alphabet_translator
                .push(to_symbols.get(&from_keys[i]).copied().unwrap_or(0));
        }
        Ok(())
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.lexicon-epsilons-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.lexicon-epsilons-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.lexicon-epsilons-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.lexicon-epsilons-fn]
    pub fn lexicon_epsilons(&mut self) {
        let front_lex = self.queue.front().unwrap().lexicon_state;
        if !self.lexicon.has_epsilons_or_flags(front_lex + 1) {
            return;
        }
        let mut next = self.lexicon.next(front_lex, 0);
        let mut i_s: STransition = self.lexicon.take_epsilons_and_flags(next);

        while i_s.symbol != NO_SYMBOL_NUMBER {
            if self.lexicon.get_transition(next).get_input_symbol() == 0 {
                let nn = self
                    .queue
                    .front()
                    .unwrap()
                    .update_lexicon(i_s.symbol, i_s.index, i_s.weight);
                self.queue.push_back(nn);
            } else {
                let mut front = self.queue.front().unwrap().clone();
                let sym = self.lexicon.get_transition(next).get_input_symbol();
                if front.flag_state.apply_operation_symbol(sym) {
                    let nn = front.update_lexicon(i_s.symbol, i_s.index, i_s.weight);
                    self.queue.push_back(nn);
                }
            }
            next += 1;
            i_s = self.lexicon.take_epsilons_and_flags(next);
        }
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.lexicon-consume-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.lexicon-consume-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.lexicon-consume-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.lexicon-consume-fn]
    pub fn lexicon_consume(&mut self) {
        let input_state = self.queue.front().unwrap().input_state;
        let front_lex = self.queue.front().unwrap().lexicon_state;
        if input_state >= self.input.len()
            || !self
                .lexicon
                .has_transitions(front_lex + 1, self.input.at(input_state))
        {
            return;
        }

        let mut next = self.lexicon.next(front_lex, self.input.at(input_state));
        let mut i_s = self
            .lexicon
            .take_non_epsilons(next, self.input.at(input_state));

        while i_s.symbol != NO_SYMBOL_NUMBER {
            let front_mut = self.queue.front().unwrap().mutator_state;
            let nn = self.queue.front().unwrap().update_input(
                i_s.symbol,
                input_state + 1,
                front_mut,
                i_s.index,
                i_s.weight,
            );
            self.queue.push_back(nn);

            next += 1;
            i_s = self
                .lexicon
                .take_non_epsilons(next, self.input.at(input_state));
        }
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.mutator-epsilons-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.mutator-epsilons-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.mutator-epsilons-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.mutator-epsilons-fn]
    pub fn mutator_epsilons(&mut self) {
        let front_mut = self.queue.front().unwrap().mutator_state;
        if !self.mutator.has_transitions(front_mut + 1, 0) {
            return;
        }
        let mut next_m = self.mutator.next(front_mut, 0);
        let mut mutator_i_s = self.mutator.take_epsilons(next_m);

        while mutator_i_s.symbol != NO_SYMBOL_NUMBER {
            if mutator_i_s.symbol == 0 {
                let nn = self.queue.front().unwrap().update_mutator(
                    mutator_i_s.symbol,
                    mutator_i_s.index,
                    mutator_i_s.weight,
                );
                self.queue.push_back(nn);
            } else {
                let front_lex = self.queue.front().unwrap().lexicon_state;
                let translated = self.alphabet_translator[mutator_i_s.symbol as usize];
                if !self.lexicon.has_transitions(front_lex + 1, translated) {
                    next_m += 1;
                    mutator_i_s = self.mutator.take_epsilons(next_m);
                    continue;
                }
                let mut next_l = self.lexicon.next(front_lex, translated);
                let mut lexicon_i_s = self.lexicon.take_non_epsilons(next_l, translated);

                while lexicon_i_s.symbol != NO_SYMBOL_NUMBER {
                    let nn = self.queue.front().unwrap().update(
                        lexicon_i_s.symbol,
                        mutator_i_s.index,
                        lexicon_i_s.index,
                        lexicon_i_s.weight + mutator_i_s.weight,
                    );
                    self.queue.push_back(nn);
                    next_l += 1;
                    lexicon_i_s = self.lexicon.take_non_epsilons(next_l, translated);
                }
            }
            next_m += 1;
            mutator_i_s = self.mutator.take_epsilons(next_m);
        }
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.consume-input-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.consume-input-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.consume-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.consume-input-fn]
    pub fn consume_input(&mut self) {
        let input_state = self.queue.front().unwrap().input_state;
        let front_mut = self.queue.front().unwrap().mutator_state;
        if input_state >= self.input.len()
            || !self
                .mutator
                .has_transitions(front_mut + 1, self.input.at(input_state))
        {
            return; // not enough input to consume or no suitable transitions
        }

        let mut next_m = self.mutator.next(front_mut, self.input.at(input_state));
        let mut mutator_i_s = self
            .mutator
            .take_non_epsilons(next_m, self.input.at(input_state));

        while mutator_i_s.symbol != NO_SYMBOL_NUMBER {
            if mutator_i_s.symbol == 0 {
                let front_lex = self.queue.front().unwrap().lexicon_state;
                let nn = self.queue.front().unwrap().update_input(
                    0,
                    input_state + 1,
                    mutator_i_s.index,
                    front_lex,
                    mutator_i_s.weight,
                );
                self.queue.push_back(nn);
            } else {
                let front_lex = self.queue.front().unwrap().lexicon_state;
                let translated = self.alphabet_translator[mutator_i_s.symbol as usize];
                if !self.lexicon.has_transitions(front_lex + 1, translated) {
                    next_m += 1;
                    mutator_i_s = self
                        .mutator
                        .take_non_epsilons(next_m, self.input.at(input_state));
                    continue;
                }
                let mut next_l = self.lexicon.next(front_lex, translated);
                let mut lexicon_i_s = self.lexicon.take_non_epsilons(next_l, translated);

                while lexicon_i_s.symbol != NO_SYMBOL_NUMBER {
                    let nn = self.queue.front().unwrap().update_input(
                        lexicon_i_s.symbol,
                        input_state + 1,
                        mutator_i_s.index,
                        lexicon_i_s.index,
                        lexicon_i_s.weight + mutator_i_s.weight,
                    );
                    self.queue.push_back(nn);
                    next_l += 1;
                    lexicon_i_s = self.lexicon.take_non_epsilons(next_l, translated);
                }
            }
            next_m += 1;
            mutator_i_s = self
                .mutator
                .take_non_epsilons(next_m, self.input.at(input_state));
        }
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.correct-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.correct-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.correct-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.correct-fn]
    pub fn correct(&mut self, line: &str) -> CorrectionQueue {
        // if input initialization fails, return empty correction queue
        let enc = self.mutator.get_encoder();
        let other = self.mutator.get_unknown_symbol();
        if !self.init_input(line, enc, other) {
            return CorrectionQueue::new();
        }
        let mut corrections: BTreeMap<String, Weight> = BTreeMap::new();
        let start_node = TreeNode::new_start(FdState::new(self.lexicon.get_fd_table()));
        self.queue.clear();
        self.queue.push_back(start_node);

        while self.queue.len() > 0 {
            self.lexicon_epsilons();
            self.mutator_epsilons();
            let input_state = self.queue.front().unwrap().input_state;
            if input_state == self.input.len() {
                // if our transducers are in final states we generate the correction
                let mutator_state = self.queue.front().unwrap().mutator_state;
                let lexicon_state = self.queue.front().unwrap().lexicon_state;
                if self.mutator.final_index(mutator_state)
                    && self.lexicon.final_index(lexicon_state)
                {
                    let string = self.stringify(self.queue.front().unwrap().string.clone());
                    let weight = self.queue.front().unwrap().weight
                        + self.lexicon.final_weight(lexicon_state)
                        + self.mutator.final_weight(mutator_state);
                    // if the correction is novel or better than before, insert it
                    if !corrections.contains_key(&string) || corrections[&string] > weight {
                        corrections.insert(string, weight);
                    }
                }
            } else {
                self.consume_input();
            }
            self.queue.pop_front();
        }
        let mut correction_queue = CorrectionQueue::new();
        for (k, v) in corrections.iter() {
            correction_queue.push((k.clone(), *v));
        }
        correction_queue
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.check-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.check-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.check-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.check-fn]
    pub fn check(&mut self, line: &str) -> bool {
        let enc = self.lexicon.get_encoder();
        if !self.init_input(line, enc, NO_SYMBOL_NUMBER) {
            return false;
        }
        let start_node = TreeNode::new_start(FdState::new(self.lexicon.get_fd_table()));
        self.queue.clear();
        self.queue.push_back(start_node);

        while self.queue.len() > 0 {
            let input_state = self.queue.front().unwrap().input_state;
            let lexicon_state = self.queue.front().unwrap().lexicon_state;
            if input_state == self.input.len() && self.lexicon.final_index(lexicon_state) {
                return true;
            }
            self.lexicon_epsilons();
            self.lexicon_consume();
            self.queue.pop_front();
        }
        false
    }

    // [spec:hfst:def:ospell.hfst-ol.speller.stringify-fn]
    // [spec:hfst:sem:ospell.hfst-ol.speller.stringify-fn]
    // [spec:hfst:def:transducer.hfst-ol.speller.stringify-fn]
    // [spec:hfst:sem:transducer.hfst-ol.speller.stringify-fn]
    pub fn stringify(&self, symbol_vector: SymbolNumberVector) -> String {
        let mut s = String::new();
        for it in symbol_vector.iter() {
            s.push_str(&self.symbol_table[*it as usize]);
        }
        s
    }
}
