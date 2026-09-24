//! Where-clause variable expansion: 'RuleVariables' and its odometer iterator.

use super::*;

impl Default for RuleVariables {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleVariables {
    pub fn new() -> Self {
        RuleVariables {
            freely_blocks: Vec::new(),
            matched_blocks: Vec::new(),
            mixed_blocks: Vec::new(),
            current_variable_block: Vec::new(),
        }
    }
}

// ===== integration: variable_src/ where-clause expansion (body[3] re-port) =====
// Ports libhfst/src/parsers/variable_src/{RuleVariables,RuleVariablesConstIterator,
// RuleSymbolVector}. The 'freely' matcher is a full cross-product; 'matched' is a
// lockstep diagonal over a block's variables (equal-size sets); 'mixed' is the
// cross-product filtered to combinations whose per-variable value POSITIONS are
// pairwise distinct (the MixedConstContainerIterator 'equal_indices' skip).
impl RuleVariables {
    // [spec:hfst:def:rule-variables.rule-variables.set-variable-fn]
    pub fn set_variable(&mut self, var: &str) {
        self.current_variable_block.push(VariableValues {
            variable: var.to_string(),
            values: Vec::new(),
        });
    }
    // [spec:hfst:def:rule-variables.rule-variables.add-value-fn]
    pub fn add_value(&mut self, value: &str) {
        if let Some(vv) = self.current_variable_block.last_mut() {
            vv.values.push(value.to_string());
        }
    }
    // [spec:hfst:def:rule-variables.rule-variables.add-values-fn]
    pub fn add_values(&mut self, values: &[String]) {
        for v in values {
            self.add_value(v);
        }
    }
    // [spec:hfst:def:rule-variables.rule-variables.set-matcher-fn]
    pub fn set_matcher(&mut self, matcher: Matcher) {
        let block = std::mem::take(&mut self.current_variable_block);
        match matcher {
            Matcher::FREELY => self.freely_blocks.push(block),
            Matcher::MATCHED => self.matched_blocks.push(block),
            Matcher::MIXED => self.mixed_blocks.push(block),
        }
    }
    // [spec:hfst:def:rule-variables.rule-variables.empty-fn]
    pub fn empty(&self) -> bool {
        self.freely_blocks.is_empty()
            && self.matched_blocks.is_empty()
            && self.mixed_blocks.is_empty()
    }
    // [spec:hfst:def:rule-variables.rule-variables.begin-fn]
    pub fn begin(&self) -> crate::error::Result<RuleVariablesConstIterator> {
        RuleVariablesConstIterator::new(self, false)
    }
    // [spec:hfst:def:rule-variables.rule-variables.end-fn]
    pub fn end(&self) -> crate::error::Result<RuleVariablesConstIterator> {
        RuleVariablesConstIterator::new(self, true)
    }
}

// One odometer dimension: a list of (variable, its values) that advance together
// (length 1 => an independent 'freely' variable; length >1 => a 'matched' block,
// lockstep over equal-size value sets). 'size' is the number of positions.
//
// For a 'mixed' block, 'combos' is non-empty: each position is a full tuple of
// per-variable value POSITIONS (pairwise distinct). For freely/matched 'combos'
// is empty and the shared position index is the value index for every variable.
#[derive(Clone)]
pub(crate) struct VarDim {
    pub(crate) vars: Vec<VariableValues>,
    pub(crate) size: usize,
    pub(crate) combos: Vec<Vec<usize>>,
}

// [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator]
pub struct RuleVariablesConstIterator {
    dims: Vec<VarDim>,
    indices: Vec<usize>,
    at_end: bool,
}

impl RuleVariablesConstIterator {
    fn new(rv: &RuleVariables, end: bool) -> crate::error::Result<Self> {
        let mut dims: Vec<VarDim> = Vec::new();
        // freely: every variable is its own independent dimension.
        for block in &rv.freely_blocks {
            for vv in block {
                dims.push(VarDim {
                    size: vv.values.len(),
                    vars: vec![vv.clone()],
                    combos: Vec::new(),
                });
            }
        }
        // matched: each block is one lockstep dimension (equal set sizes required).
        for block in &rv.matched_blocks {
            let size = block.first().map(|v| v.values.len()).unwrap_or(0);
            for vv in block {
                if vv.values.len() != size {
                    crate::bail!(UnequalSetSize);
                }
            }
            dims.push(VarDim {
                size,
                vars: block.clone(),
                combos: Vec::new(),
            });
        }
        // mixed: one dimension whose positions are the cross-product of the
        // block's per-variable value POSITIONS, keeping only the tuples whose
        // positions are pairwise distinct (the C++ 'equal_indices' skip). The
        // odometer below advances position 0 fastest, mirroring
        // ConstContainerIterator::operator++.
        for block in &rv.mixed_blocks {
            let lens: Vec<usize> = block.iter().map(|vv| vv.values.len()).collect();
            let n = lens.len();
            let mut combos: Vec<Vec<usize>> = Vec::new();
            if n != 0 && lens.iter().all(|&l| l != 0) {
                let mut idx = vec![0usize; n];
                loop {
                    let mut seen: std::collections::BTreeSet<usize> =
                        std::collections::BTreeSet::new();
                    let mut distinct = true;
                    for &k in &idx {
                        if !seen.insert(k) {
                            distinct = false;
                            break;
                        }
                    }
                    if distinct {
                        combos.push(idx.clone());
                    }
                    // advance the mixed-radix odometer (position 0 fastest)
                    let mut i = 0;
                    while i < n {
                        idx[i] += 1;
                        if idx[i] < lens[i] {
                            break;
                        }
                        idx[i] = 0;
                        i += 1;
                    }
                    if i == n {
                        break;
                    }
                }
            }
            dims.push(VarDim {
                size: combos.len(),
                vars: block.clone(),
                combos,
            });
        }
        let any_empty = dims.iter().any(|d| d.size == 0);
        let indices = vec![0usize; dims.len()];
        Ok(RuleVariablesConstIterator {
            dims,
            indices,
            at_end: end || any_empty,
        })
    }

    // [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.set-values-fn]
    pub fn set_values(&self, vvm: &mut VariableValueMap) {
        for (dim, &idx) in self.dims.iter().zip(self.indices.iter()) {
            if dim.combos.is_empty() {
                // freely / matched: every variable shares the position index.
                for vv in &dim.vars {
                    vvm.insert(vv.variable.clone(), vv.values[idx].clone());
                }
            } else {
                // mixed: each variable takes its own position from the tuple.
                let combo = &dim.combos[idx];
                for (k, vv) in dim.vars.iter().enumerate() {
                    vvm.insert(vv.variable.clone(), vv.values[combo[k]].clone());
                }
            }
        }
    }

    pub fn increment(&mut self) {
        if self.at_end {
            return;
        }
        // odometer: advance the rightmost dimension, carry to the left.
        let mut i = self.dims.len();
        loop {
            if i == 0 {
                self.at_end = true;
                return;
            }
            i -= 1;
            self.indices[i] += 1;
            if self.indices[i] < self.dims[i].size {
                return;
            }
            self.indices[i] = 0;
        }
    }

    pub fn ne(&self, other: &RuleVariablesConstIterator) -> bool {
        self.at_end != other.at_end || (!self.at_end && self.indices != other.indices)
    }
}
