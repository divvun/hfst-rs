//! Port of 'libhfst/src/HfstLookupFlagDiacritics.{h,cc}'.
//!
//! 'FlagDiacriticTable''s 'diacritic_*' maps were C++ 'static' class members —
//! process-global mutable state — used as a memo-cache: 'split_diacritic' parsed
//! a diacritic string into them once and the accessors read them back. Because
//! that parse is a deterministic pure function of the string, the cache is gone:
//! 'parse_diacritic' recomputes the (operator, feature, value) on demand, so the
//! type holds no global state. The only mutable state left is each instance's own
//! 'feature_values'/'feature_polarities' (the actual flag-unification registers).
//!
//! The '#ifdef DEBUG' 'main' is dead code (it calls a
//! 'define_diacritic'/'KeyVector' API that no longer exists) and is not ported.
//! The '#ifdef DEBUG' 'display' is ported; see its note.

use std::collections::BTreeMap;

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

// The C++ 'static' class members (diacritic_operators/_features/_values/
// _has_value) were a process-global memo-cache that 'split_diacritic' populated
// from the diacritic string and the accessors below read back. The parse is a
// deterministic pure function of the string, so the cache is gone: the accessors
// recompute via 'parse_diacritic' on demand. Four shared-mutable globals removed.

// [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table]
pub struct FlagDiacriticTable {
    feature_values: FeatureValues,
    feature_polarities: FeaturePolarities,
    error_flag: bool,
}

impl FlagDiacriticTable {
    // Decompose a flag diacritic '@[PNDRCU].FEATURE(.VALUE)?@' into (operator,
    // feature, value). This is exactly the parse the C++ 'split_diacritic'
    // performed once into the global maps; it is recomputed on demand instead.
    // For a non-diacritic string the accessors below never see it (callers gate
    // on 'is_diacritic' first), but it returns inert defaults rather than panic.
    fn parse_diacritic(symbol: &str) -> (DiacriticOperator, String, Option<String>) {
        let bytes = symbol.as_bytes();
        if symbol.len() < 5 || bytes[2] != b'.' {
            return (DiacriticOperator::Pop, String::new(), None);
        }
        let op = match bytes[1] {
            b'P' => DiacriticOperator::Pop,
            b'N' => DiacriticOperator::Nop,
            b'D' => DiacriticOperator::Dop,
            b'R' => DiacriticOperator::Rop,
            b'C' => DiacriticOperator::Cop,
            b'U' => DiacriticOperator::Uop,
            _ => DiacriticOperator::Pop,
        };
        // Third character is always the first fullstop (index 2).
        let first_full_stop_pos: usize = 2;
        let second_full_stop_pos = symbol[first_full_stop_pos + 1..]
            .find('.')
            .map(|i| i + first_full_stop_pos + 1);
        let last_char_pos = symbol.len() - 1;
        match second_full_stop_pos {
            None => (
                op,
                symbol[first_full_stop_pos + 1..last_char_pos].to_string(),
                None,
            ),
            Some(second_full_stop_pos) => (
                op,
                symbol[first_full_stop_pos + 1..second_full_stop_pos].to_string(),
                Some(symbol[second_full_stop_pos + 1..last_char_pos].to_string()),
            ),
        }
    }

    // Accessors mirroring 'operator[]' on the (now-removed) static maps.
    fn op_of(symbol: &str) -> DiacriticOperator {
        Self::parse_diacritic(symbol).0
    }
    fn feature_of(symbol: &str) -> String {
        Self::parse_diacritic(symbol).1
    }
    fn value_of(symbol: &str) -> String {
        Self::parse_diacritic(symbol).2.unwrap_or_default()
    }
    fn has_value_of(symbol: &str) -> bool {
        Self::parse_diacritic(symbol).2.is_some()
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
        if diacritic_string.rfind('.') == Some(2)
            && bytes[1] != b'R'
            && bytes[1] != b'D'
            && bytes[1] != b'C'
        {
            return false;
        }
        true
    }

    // The C++ 'split_diacritic' parsed a genuine diacritic into the global maps;
    // that work now lives in 'parse_diacritic', recomputed on demand by the
    // accessors, so the cache-populating pass is gone. Its 'assert' that a
    // value-less diacritic is C/D/R-op is preserved by 'is_genuine_diacritic'
    // (which rejects '@P.X@' / '@N.X@' / '@U.X@' — they require a value).

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
        // The C++ also eagerly split a genuine diacritic into the global maps;
        // with on-demand parsing that side effect is unnecessary.
        Self::is_genuine_diacritic(symbol)
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
        if !self.feature_values.contains_key(feature)
            || self.feature_values[feature].as_str() != value
        {
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
    // string. With the cache gone, "defined" is just "parses as a genuine
    // diacritic"; the op/feature/value come from the on-demand accessors, and the
    // unscoped C++ enum streams as its integer value (mirrored with 'as i32').
    pub fn display(diacritic: i16) {
        let key = diacritic.to_string();
        if !Self::is_genuine_diacritic(&key) {
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
