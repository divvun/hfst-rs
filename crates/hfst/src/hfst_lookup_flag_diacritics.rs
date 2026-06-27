//! Port of 'libhfst/src/HfstLookupFlagDiacritics.{h,cc}'.
//!
//! 'FlagDiacriticTable''s 'diacritic_*' maps are C++ 'static' class members, i.e.
//! process-global mutable state shared across all instances. They are ported as
//! module-level 'static Mutex<BTreeMap<…>>'. C++ 'map[key]' reads are ported as
//! 'entry(...).or_default()' / '.or_insert(...)' to preserve 'operator[]''s
//! default-insert side effect.
//!
//! The '#ifdef DEBUG' 'main' is dead code (it calls a
//! 'define_diacritic'/'KeyVector' API that no longer exists) and is not ported.
//! The '#ifdef DEBUG' 'display' is ported; see its note for the 'short'-key
//! adaptation forced by the now-'String'-keyed static maps.

use std::collections::BTreeMap;
use std::sync::Mutex;

// [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-operator]
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum DiacriticOperator {
    #[default]
    Pop,
    Nop,
    Dop,
    Rop,
    Cop,
    Uop,
}

// [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-operators]
pub type DiacriticOperators = BTreeMap<String, DiacriticOperator>;
// [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-features]
pub type DiacriticFeatures = BTreeMap<String, String>;
// [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-values]
pub type DiacriticValues = BTreeMap<String, String>;
// [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-setting-map]
pub type DiacriticSettingMap = BTreeMap<String, bool>;
// [spec:hfst:def:hfst-lookup-flag-diacritics.feature-values]
pub type FeatureValues = BTreeMap<String, String>;
// [spec:hfst:def:hfst-lookup-flag-diacritics.feature-polarities]
pub type FeaturePolarities = BTreeMap<String, bool>;

// [spec:hfst:def:hfst-lookup-flag-diacritics.hfst.string-vector]
pub use crate::hfst_data_types::StringVector;

// The 'static' class members of FlagDiacriticTable.
static DIACRITIC_OPERATORS: Mutex<DiacriticOperators> = Mutex::new(BTreeMap::new());
static DIACRITIC_FEATURES: Mutex<DiacriticFeatures> = Mutex::new(BTreeMap::new());
static DIACRITIC_VALUES: Mutex<DiacriticValues> = Mutex::new(BTreeMap::new());
static DIACRITIC_HAS_VALUE: Mutex<DiacriticSettingMap> = Mutex::new(BTreeMap::new());

// [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table]
pub struct FlagDiacriticTable {
    feature_values: FeatureValues,
    feature_polarities: FeaturePolarities,
    error_flag: bool,
}

impl FlagDiacriticTable {
    // Accessors mirroring 'operator[]' on the static maps (default-insert).
    fn op_of(symbol: &str) -> DiacriticOperator {
        *DIACRITIC_OPERATORS
            .lock()
            .unwrap()
            .entry(symbol.to_string())
            .or_insert(DiacriticOperator::Pop)
    }
    fn feature_of(symbol: &str) -> String {
        DIACRITIC_FEATURES
            .lock()
            .unwrap()
            .entry(symbol.to_string())
            .or_default()
            .clone()
    }
    fn value_of(symbol: &str) -> String {
        DIACRITIC_VALUES
            .lock()
            .unwrap()
            .entry(symbol.to_string())
            .or_default()
            .clone()
    }
    fn has_value_of(symbol: &str) -> bool {
        *DIACRITIC_HAS_VALUE
            .lock()
            .unwrap()
            .entry(symbol.to_string())
            .or_default()
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.is-genuine-diacritic-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.is-genuine-diacritic-fn]
    fn is_genuine_diacritic(diacritic_string: &str) -> bool {
        let bytes = diacritic_string.as_bytes();
        // All diacritics have form @[A-Z][.][A-Z]+([.][A-Z]+)?@
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

    // Precondition: diacritic_string matches @[A-Z][.][A-Z]+([.][A-Z]+)?@
    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.split-diacritic-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.split-diacritic-fn]
    fn split_diacritic(diacritic_string: &str) {
        match diacritic_string.as_bytes()[1] {
            b'P' => {
                DIACRITIC_OPERATORS
                    .lock()
                    .unwrap()
                    .insert(diacritic_string.to_string(), DiacriticOperator::Pop);
            }
            b'N' => {
                DIACRITIC_OPERATORS
                    .lock()
                    .unwrap()
                    .insert(diacritic_string.to_string(), DiacriticOperator::Nop);
            }
            b'D' => {
                DIACRITIC_OPERATORS
                    .lock()
                    .unwrap()
                    .insert(diacritic_string.to_string(), DiacriticOperator::Dop);
            }
            b'R' => {
                DIACRITIC_OPERATORS
                    .lock()
                    .unwrap()
                    .insert(diacritic_string.to_string(), DiacriticOperator::Rop);
            }
            b'C' => {
                DIACRITIC_OPERATORS
                    .lock()
                    .unwrap()
                    .insert(diacritic_string.to_string(), DiacriticOperator::Cop);
            }
            b'U' => {
                DIACRITIC_OPERATORS
                    .lock()
                    .unwrap()
                    .insert(diacritic_string.to_string(), DiacriticOperator::Uop);
            }
            _ => {
                assert!(false);
            }
        }

        // Third character is always the first fullstop.
        let first_full_stop_pos: usize = 2;
        // Find the second full stop, if there is one.
        let second_full_stop_pos = diacritic_string[first_full_stop_pos + 1..]
            .find('.')
            .map(|i| i + first_full_stop_pos + 1);
        let last_char_pos = diacritic_string.len() - 1;
        match second_full_stop_pos {
            None => {
                let op = Self::op_of(diacritic_string);
                assert!(
                    op == DiacriticOperator::Cop
                        || op == DiacriticOperator::Dop
                        || op == DiacriticOperator::Rop
                );
                DIACRITIC_HAS_VALUE
                    .lock()
                    .unwrap()
                    .insert(diacritic_string.to_string(), false);
                DIACRITIC_FEATURES.lock().unwrap().insert(
                    diacritic_string.to_string(),
                    diacritic_string[first_full_stop_pos + 1..last_char_pos].to_string(),
                );
            }
            Some(second_full_stop_pos) => {
                DIACRITIC_HAS_VALUE
                    .lock()
                    .unwrap()
                    .insert(diacritic_string.to_string(), true);
                DIACRITIC_FEATURES.lock().unwrap().insert(
                    diacritic_string.to_string(),
                    diacritic_string[first_full_stop_pos + 1..second_full_stop_pos].to_string(),
                );
                DIACRITIC_VALUES.lock().unwrap().insert(
                    diacritic_string.to_string(),
                    diacritic_string[second_full_stop_pos + 1..last_char_pos].to_string(),
                );
            }
        }
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.flag-diacritic-table-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.flag-diacritic-table-fn]
    pub fn new() -> Self {
        FlagDiacriticTable {
            feature_values: BTreeMap::new(),
            feature_polarities: BTreeMap::new(),
            error_flag: false,
        }
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.is-diacritic-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.is-diacritic-fn]
    pub fn is_diacritic(symbol: &str) -> bool {
        let res = Self::is_genuine_diacritic(symbol);
        if res {
            Self::split_diacritic(symbol);
        }
        res
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.set-positive-value-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.set-positive-value-fn]
    fn set_positive_value(&mut self, feature: &str, value: &str) {
        self.feature_values
            .insert(feature.to_string(), value.to_string());
        self.feature_polarities.insert(feature.to_string(), true);
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.set-negative-value-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.set-negative-value-fn]
    fn set_negative_value(&mut self, feature: &str, value: &str) {
        self.feature_values
            .insert(feature.to_string(), value.to_string());
        self.feature_polarities.insert(feature.to_string(), false);
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.disallow-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.disallow-fn]
    fn disallow(&mut self, feature: &str, value: &str) {
        if !self.feature_values.contains_key(feature) {
            return;
        }
        if self.feature_values[feature].as_str() == value {
            let pol = *self
                .feature_polarities
                .entry(feature.to_string())
                .or_insert(false);
            self.error_flag = self.error_flag || pol;
        }
    }

    fn disallow_feature(&mut self, feature: &str) {
        if self.feature_values.contains_key(feature) {
            self.error_flag = true;
        }
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.require-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.require-fn]
    fn require(&mut self, feature: &str, value: &str) {
        if !self.feature_values.contains_key(feature) {
            self.error_flag = true;
            return;
        } else if self.feature_values[feature].as_str() != value {
            self.error_flag = true;
        } else {
            let pol = *self
                .feature_polarities
                .entry(feature.to_string())
                .or_insert(false);
            self.error_flag = self.error_flag || (!pol);
        }
    }

    fn require_feature(&mut self, feature: &str) {
        if !self.feature_values.contains_key(feature) {
            self.error_flag = true;
        }
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.unify-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.unify-fn]
    fn unify(&mut self, feature: &str, value: &str) {
        // If feature not set, set it to value.
        if !self.feature_values.contains_key(feature) {
            self.set_positive_value(feature, value);
        }
        // If feature set to something else negatively, set it to value.
        else if self.feature_values[feature].as_str() != value {
            let pol = *self
                .feature_polarities
                .entry(feature.to_string())
                .or_insert(false);
            if !pol {
                self.set_positive_value(feature, value);
            }
        }
        self.require(feature, value);
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.clear-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.clear-fn]
    fn clear(&mut self, feature: &str) {
        self.feature_values.remove(feature);
        self.feature_polarities.remove(feature);
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.insert-symbol-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.insert-symbol-fn]
    pub fn insert_symbol(&mut self, symbol: &str) {
        if Self::is_diacritic(symbol) {
            match Self::op_of(symbol) {
                DiacriticOperator::Pop => {
                    let f = Self::feature_of(symbol);
                    let v = Self::value_of(symbol);
                    self.set_positive_value(&f, &v);
                }
                DiacriticOperator::Nop => {
                    let f = Self::feature_of(symbol);
                    let v = Self::value_of(symbol);
                    self.set_negative_value(&f, &v);
                }
                DiacriticOperator::Dop => {
                    if !Self::has_value_of(symbol) {
                        let f = Self::feature_of(symbol);
                        self.disallow_feature(&f);
                    } else {
                        let f = Self::feature_of(symbol);
                        let v = Self::value_of(symbol);
                        self.disallow(&f, &v);
                    }
                }
                DiacriticOperator::Rop => {
                    if !Self::has_value_of(symbol) {
                        let f = Self::feature_of(symbol);
                        self.require_feature(&f);
                    } else {
                        let f = Self::feature_of(symbol);
                        let v = Self::value_of(symbol);
                        self.require(&f, &v);
                    }
                }
                DiacriticOperator::Cop => {
                    let f = Self::feature_of(symbol);
                    self.clear(&f);
                }
                DiacriticOperator::Uop => {
                    let f = Self::feature_of(symbol);
                    let v = Self::value_of(symbol);
                    self.unify(&f, &v);
                }
            }
        }
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.fails-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.fails-fn]
    pub fn fails(&self) -> bool {
        self.error_flag
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.reset-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.reset-fn]
    pub fn reset(&mut self) {
        self.error_flag = false;
        self.feature_values.clear();
        self.feature_polarities.clear();
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.is-valid-string-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.is-valid-string-fn]
    pub fn is_valid_string(&mut self, input_string: &StringVector) -> bool {
        self.reset();
        for it in input_string.iter() {
            self.insert_symbol(it);
            if self.fails() {
                return false;
            }
        }
        true
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.filter-diacritics-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.filter-diacritics-fn]
    pub fn filter_diacritics(&self, input_string: &StringVector) -> StringVector {
        let mut filtered = StringVector::new();
        for it in input_string.iter() {
            if !Self::is_diacritic(it) {
                filtered.push(it.clone());
            }
        }
        filtered
    }

    // [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.display-fn]
    // [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.display-fn]
    //
    // '#ifdef DEBUG' dead code: the original keyed the static maps by 'short'
    // (an old number-based API), so the 'short' key is looked up by its decimal
    // string in the surviving 'String'-keyed maps. 'operator[]''s default-insert
    // is preserved via the 'op_of'/'feature_of'/'value_of' accessors, and the
    // unscoped C++ enum streams as its integer value (mirrored with 'as i32').
    pub fn display(diacritic: i16) {
        let key = diacritic.to_string();
        if !DIACRITIC_OPERATORS.lock().unwrap().contains_key(&key) {
            println!("{} not defined.", diacritic);
        } else {
            println!(
                "{} {} {}",
                Self::op_of(&key) as i32,
                Self::feature_of(&key),
                Self::value_of(&key)
            );
        }
    }
}

impl Default for FlagDiacriticTable {
    fn default() -> Self {
        Self::new()
    }
}
