//! Port of 'libhfst/src/HfstFlagDiacritics.{h,cc}' — flag diacritic handling.
//!
//! 'FdTable<T>' and 'FdState<T>' are C++ class templates; here they are Rust
//! generics over 'T: Ord + Clone' (the symbol-key type). 'FdState' keeps the
//! C++ 'const FdTable<T>*' as a raw '*const FdTable<T>' (nullable, as the C++
//! default constructor leaves it 'NULL'); table dereferences are 'unsafe',
//! mirroring the original pointer contract (the borrowed table outlives the
//! state).

use std::collections::BTreeMap;

use crate::hfst_data_types::size_t_to_ushort;

// [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operator]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FdOperator {
    Pop,
    Nop,
    Rop,
    Dop,
    Cop,
    Uop,
}

// [spec:hfst:def:hfst-flag-diacritics.hfst.fd-feature]
pub type FdFeature = u16;
// [spec:hfst:def:hfst-flag-diacritics.hfst.fd-value]
pub type FdValue = i16;

// 'std::string::npos'.
const NPOS: usize = usize::MAX;

// [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation]
#[derive(Clone, Debug)]
pub struct FdOperation {
    op: FdOperator,
    feature: FdFeature,
    value: FdValue,
    name: String,
}

impl FdOperation {
    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.fd-operation-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.fd-operation-fn]
    pub fn new(op: FdOperator, feat: FdFeature, val: FdValue, str: &str) -> Self {
        FdOperation {
            op,
            feature: feat,
            value: val,
            name: str.to_string(),
        }
    }

    // Required for operator[]() — the default-constructed FdOperation.
    pub fn new_default() -> Self {
        FdOperation {
            op: FdOperator::Pop,
            feature: 0,
            value: 0,
            name: String::new(),
        }
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.operator-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.operator-fn]
    pub fn Operator(&self) -> FdOperator {
        self.op
    }
    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.feature-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.feature-fn]
    pub fn Feature(&self) -> FdFeature {
        self.feature
    }
    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.value-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.value-fn]
    pub fn Value(&self) -> FdValue {
        self.value
    }
    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.name-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.name-fn]
    pub fn Name(&self) -> String {
        self.name.clone()
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.char-to-operator-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.char-to-operator-fn]
    //
    // The C++ 'default: throw;' rethrows with no active exception, i.e. calls
    // 'std::terminate'; here that is a 'panic!'.
    pub fn char_to_operator(c: char) -> FdOperator {
        match c {
            'P' => FdOperator::Pop,
            'N' => FdOperator::Nop,
            'R' => FdOperator::Rop,
            'D' => FdOperator::Dop,
            'C' => FdOperator::Cop,
            'U' => FdOperator::Uop,
            _ => panic!("FdOperation::char_to_operator: not a flag operator"),
        }
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.is-diacritic-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.is-diacritic-fn]
    //
    // All diacritics have form @[PNDRCU][.][A-Z]+([.][A-Z]+)?@. Indexing is by
    // byte, matching C++ 'std::string::at'/'size'/'find_last_of' over the ASCII
    // diacritic syntax.
    pub fn is_diacritic(diacritic_string: &str) -> bool {
        let bytes = diacritic_string.as_bytes();
        if diacritic_string.len() < 5 {
            return false;
        }
        if bytes[2] != b'.' {
            return false;
        }
        // These two checks probably always succeed...
        if bytes[0] != b'@' {
            return false;
        }
        if bytes[diacritic_string.len() - 1] != b'@' {
            return false;
        }
        match bytes[1] {
            b'P' => {}
            b'N' => {}
            b'D' => {}
            b'R' => {}
            b'C' => {}
            b'U' => {}
            _ => return false,
        }
        if diacritic_string.rfind('.') == Some(2) {
            if bytes[1] != b'R' && bytes[1] != b'D' && bytes[1] != b'C' {
                return false;
            }
        }
        true
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.find-diacritic-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.find-diacritic-fn]
    pub fn find_diacritic(diacritic_str: &str, length: &mut usize) -> usize {
        let start = diacritic_str.find('@');
        if let Some(start) = start {
            let end = diacritic_str[start + 1..].find('@').map(|i| i + start + 1);
            if let Some(end) = end {
                // is_diacritic(diacritic_str.substr(start, end-start))
                if Self::is_diacritic(&diacritic_str[start..end]) {
                    *length = end - start;
                    return start;
                }
            }
        }
        NPOS
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.get-operator-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.get-operator-fn]
    pub fn get_operator(diacritic: &str) -> String {
        // The operator is the second char.
        diacritic[1..2].to_string()
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.get-feature-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.get-feature-fn]
    pub fn get_feature(diacritic: &str) -> String {
        // The feature name starts after the '@', '.' and operator chars.
        let feature_start = 3;
        // The feature name ends at the '.' char after the feature name start pos.
        let feature_past = match diacritic[3..].find('.').map(|i| i + 3) {
            Some(p) => p,
            // If there is no value given (e.g. "@D.FOO@"), point to the last '@'.
            None => diacritic.len() - 1,
        };
        diacritic[feature_start..feature_past].to_string()
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.get-value-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.get-value-fn]
    pub fn get_value(diacritic: &str) -> String {
        // First locate the second '.' char.
        let second_comma = match diacritic.find('.') {
            Some(i) => diacritic[i + 1..].find('.').map(|j| j + i + 1),
            None => None,
        };
        // If there is no second '.' char (e.g. "@D.FOO@"), return an empty string.
        let second_comma = match second_comma {
            None => return String::new(),
            Some(s) => s,
        };
        // The value starts after the second '.' char.
        let value_start = second_comma + 1;
        // The value ends at the last char.
        let value_past = diacritic.len() - 1;
        diacritic[value_start..value_past].to_string()
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.has-value-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.has-value-fn]
    //
    // True iff there is a second '.' in the diacritic. The C++ computes
    // 'find('.', find('.') + 1)'; when there is no first '.', its 'npos + 1'
    // wraps to a search from 0 that still finds none — the same false result.
    pub fn has_value(flag_diacritic: &str) -> bool {
        match flag_diacritic.find('.') {
            Some(i) => flag_diacritic[i + 1..].find('.').is_some(),
            None => false,
        }
    }
}

/// \brief A collection of the flag diacritics from a symbol table indexed by
/// keys of type 'T'.
// [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table]
#[derive(Clone)]
pub struct FdTable<T: Ord + Clone> {
    // Used for generating IDs that stand in for feature and value strings
    feature_map: BTreeMap<String, FdFeature>,
    value_map: BTreeMap<String, FdValue>,

    operations: BTreeMap<T, FdOperation>,
    symbol_map: BTreeMap<String, T>,
}

impl<T: Ord + Clone> FdTable<T> {
    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.fd-table-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.fd-table-fn]
    pub fn new() -> Self {
        let mut value_map = BTreeMap::new();
        value_map.insert(String::new(), 0); // empty value = neutral
        FdTable {
            feature_map: BTreeMap::new(),
            value_map,
            operations: BTreeMap::new(),
            symbol_map: BTreeMap::new(),
        }
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.define-diacritic-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.define-diacritic-fn]
    pub fn define_diacritic(&mut self, symbol: T, str: &str) {
        if !FdOperation::is_diacritic(str) {
            panic!("FdTable::define_diacritic: not a diacritic");
        }

        let op = FdOperation::char_to_operator(str.as_bytes()[1] as char);
        let feat: String;
        let mut val: String = String::new();

        // Third character is always the first fullstop.
        let first_full_stop_pos: usize = 2;
        // Find the second full stop, if there is one.
        let second_full_stop_pos = str[first_full_stop_pos + 1..]
            .find('.')
            .map(|i| i + first_full_stop_pos + 1);
        let last_char_pos = str.len() - 1;
        match second_full_stop_pos {
            None => {
                assert!(op == FdOperator::Cop || op == FdOperator::Dop || op == FdOperator::Rop);
                // substr(first_full_stop_pos+1, last_char_pos-first_full_stop_pos-1)
                feat = str[first_full_stop_pos + 1..last_char_pos].to_string();
            }
            Some(second_full_stop_pos) => {
                feat = str[first_full_stop_pos + 1..second_full_stop_pos].to_string();
                // substr(second_full_stop_pos+1, last_char_pos-second_full_stop_pos-1)
                val = str[second_full_stop_pos + 1..last_char_pos].to_string();
            }
        }

        if !self.feature_map.contains_key(&feat) {
            let next: FdFeature = size_t_to_ushort(self.feature_map.len());
            self.feature_map.insert(feat.clone(), next);
        }
        if !self.value_map.contains_key(&val) {
            let next: FdValue = size_t_to_ushort(self.value_map.len() + 1) as FdValue;
            self.value_map.insert(val.clone(), next);
        }

        let operation = FdOperation::new(op, self.feature_map[&feat], self.value_map[&val], str);
        self.operations.insert(symbol.clone(), operation);
        self.symbol_map.insert(str.to_string(), symbol);
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.num-features-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.num-features-fn]
    pub fn num_features(&self) -> FdFeature {
        self.feature_map.len() as FdFeature
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.is-diacritic-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.is-diacritic-fn]
    pub fn is_diacritic(&self, symbol: T) -> bool {
        self.operations.contains_key(&symbol)
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.get-symbols-with-feature-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.get-symbols-with-feature-fn]
    pub fn get_symbols_with_feature(&self, feature: &str) -> Vec<T> {
        let mut retval: Vec<T> = Vec::new();
        if !self.feature_map.contains_key(feature) {
            return retval;
        }
        let feature_code = self.feature_map[feature];
        for (key, opn) in self.operations.iter() {
            if opn.Feature() == feature_code {
                retval.push(key.clone());
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.get-operation-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.get-operation-fn]
    //
    // The C++ returns 'const FdOperation*' (NULL when absent); ported as
    // 'Option<&FdOperation>'.
    pub fn get_operation(&self, symbol: T) -> Option<&FdOperation> {
        self.operations.get(&symbol)
    }

    pub fn get_operation_by_string(&self, symbol: &str) -> Option<&FdOperation> {
        match self.symbol_map.get(symbol) {
            None => None,
            Some(t) => self.get_operation(t.clone()),
        }
    }

    pub fn is_valid_string_symbols(&self, symbols: &Vec<T>) -> bool {
        let mut state: FdState<T> = FdState::new(self);

        for i in 0..symbols.len() {
            if !state.apply_operation_symbol(symbols[i].clone()) {
                break;
            }
        }
        !state.fails()
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.is-valid-string-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.is-valid-string-fn]
    pub fn is_valid_string(&self, str: &str) -> bool {
        let mut state: FdState<T> = FdState::new(self);
        let mut remaining: String = str.to_string();
        let mut length: usize = 0;

        loop {
            let next_diacritic_pos = FdOperation::find_diacritic(&remaining, &mut length);
            if next_diacritic_pos == NPOS {
                break;
            }

            let diacritic = remaining[0..length].to_string();
            if !state.apply_operation_string(&diacritic) {
                break;
            }
            remaining = remaining[length..].to_string();
        }
        !state.fails()
    }
}

impl<T: Ord + Clone> Default for FdTable<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// \brief Contains the values of each of the flag diacritic features from a
/// table. It allows for evaluating a series of diacritic operations.
// [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state]
// SAFETY-ISLAND [fdstate-self-ref]: `table` borrows the `FdTable` that the owning
// structs (hfst-ol `Transducer`, `PmatchContainer`, the lookup state) also hold
// by value — so a `&'a FdTable` field would make those structs self-referential,
// which safe Rust can't express without Rc/arena. The pointer is set from a live
// `&FdTable` in `new` (null in `new_default`, never dereferenced then), the table
// outlives the `FdState`, and every deref below is a shared read.
#[derive(Clone)]
pub struct FdState<T: Ord + Clone> {
    table: *const FdTable<T>,

    // This is indexed with values of type FdFeature
    values: Vec<FdValue>,
    // C++ types this 'T'; it always holds a feature count (a 'FdFeature'), so it
    // is typed as such here.
    num_features: FdFeature,

    error_flag: bool,
}

impl<T: Ord + Clone> FdState<T> {
    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.fd-state-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.fd-state-fn]
    pub fn new(t: &FdTable<T>) -> Self {
        FdState {
            table: t as *const FdTable<T>,
            values: vec![0 as FdValue; t.num_features() as usize],
            num_features: t.num_features(),
            error_flag: false,
        }
    }

    pub fn new_default() -> Self {
        FdState {
            table: std::ptr::null(),
            values: Vec::new(),
            num_features: 0,
            error_flag: false,
        }
    }

    pub fn get_table(&self) -> &FdTable<T> {
        unsafe { &*self.table }
    }

    pub fn get_values(&self) -> &Vec<FdValue> {
        &self.values
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.assign-values-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.assign-values-fn]
    pub fn assign_values(&mut self, vals: &Vec<FdValue>) {
        self.values = vals.clone();
        if self.values.len() != self.num_features as usize {
            self.error_flag = true;
        }
    }

    pub fn apply_operation_symbol(&mut self, symbol: T) -> bool {
        let op = unsafe { (*self.table).get_operation(symbol) };
        if let Some(op) = op {
            let op = op.clone();
            return self.apply_operation(&op);
        }
        true // if the symbol isn't a diacritic
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.apply-operation-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.apply-operation-fn]
    pub fn apply_operation(&mut self, op: &FdOperation) -> bool {
        match op.Operator() {
            FdOperator::Pop => {
                // positive set
                self.values[op.Feature() as usize] = op.Value();
                true
            }
            FdOperator::Nop => {
                // negative set (literally, in this implementation)
                self.values[op.Feature() as usize] = -1 * op.Value();
                true
            }
            FdOperator::Rop => {
                // require
                if op.Value() == 0 {
                    // empty require
                    self.values[op.Feature() as usize] != 0
                } else {
                    // nonempty require
                    self.values[op.Feature() as usize] == op.Value()
                }
            }
            FdOperator::Dop => {
                // disallow
                if op.Value() == 0 {
                    // empty disallow
                    self.values[op.Feature() as usize] == 0
                } else {
                    // nonempty disallow
                    self.values[op.Feature() as usize] != op.Value()
                }
            }
            FdOperator::Cop => {
                // clear
                self.values[op.Feature() as usize] = 0;
                true
            }
            FdOperator::Uop => {
                // unification
                let f = op.Feature() as usize;
                if self.values[f] == 0 /* if the feature is unset or */
                    || self.values[f] == op.Value() /* at this value already or */
                    || (self.values[f] < 0 && (self.values[f] * (-1) != op.Value()))
                /* negatively set to something else */
                {
                    self.values[f] = op.Value();
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn apply_operation_string(&mut self, symbol: &str) -> bool {
        let op = unsafe { (*self.table).get_operation_by_string(symbol) };
        if let Some(op) = op {
            let op = op.clone();
            return self.apply_operation(&op);
        }
        true
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.fails-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.fails-fn]
    pub fn fails(&self) -> bool {
        self.error_flag
    }

    // [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.reset-fn]
    // [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.reset-fn]
    pub fn reset(&mut self) {
        self.error_flag = false;
        self.values.clear();
        let nf = unsafe { (*self.table).num_features() } as usize;
        self.values.resize(nf, 0);
    }
}
