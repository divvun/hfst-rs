//! The optimized-lookup alphabet: symbol table, flag diacritics and special symbols.

use super::*;
use crate::hfst_symbol_defs::{is_default, is_identity, is_unknown};

// [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.unicode-class-cache-value]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnicodeClassCacheValue {
    upperalpha,
    loweralpha,
    whitespace,
    no_value,
    other,
}

/// The Unicode class of a symbol spelling's first character, computed rather
/// than looked up.
///
/// [`TransducerAlphabet::cache_unicode_class`] memoizes the same answer into the
/// alphabet, which makes asking the question a write to a structure that is
/// otherwise fixed at load. A caller holding the alphabet shared computes the
/// class from the spelling and keeps its own memo.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
pub(crate) fn unicode_class_of(spelling: &str) -> UnicodeClassCacheValue {
    let Some(c) = spelling.chars().next() else {
        return UnicodeClassCacheValue::no_value;
    };
    if c.is_lowercase() {
        UnicodeClassCacheValue::loweralpha
    } else if c.is_uppercase() {
        UnicodeClassCacheValue::upperalpha
    } else if c.is_whitespace() {
        UnicodeClassCacheValue::whitespace
    } else {
        UnicodeClassCacheValue::other
    }
}

// [spec:hfst:def:transducer.hfst-ol.transducer-alphabet]
#[derive(Clone)]
pub struct TransducerAlphabet {
    pub(crate) symbol_table: SymbolTable,
    pub(crate) fd_table: FdTable<SymbolNumber>,
    pub(crate) unknown_symbol: SymbolNumber,
    pub(crate) default_symbol: SymbolNumber,
    pub(crate) identity_symbol: SymbolNumber,
    pub(crate) orig_symbol_count: SymbolNumber,
    unicode_cache: Vec<UnicodeClassCacheValue>,
}

impl TransducerAlphabet {
    pub fn new() -> Self {
        let symbol_table = vec![Symbol::new_static("@_EPSILON_SYMBOL_@")];
        TransducerAlphabet {
            symbol_table,
            fd_table: FdTable::new(),
            unknown_symbol: NO_SYMBOL_NUMBER,
            default_symbol: NO_SYMBOL_NUMBER,
            identity_symbol: NO_SYMBOL_NUMBER,
            orig_symbol_count: 1,
            unicode_cache: Vec::new(),
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.transducer-alphabet-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.transducer-alphabet-fn]
    pub fn read_from(
        is: &mut dyn std::io::BufRead,
        symbol_count: SymbolNumber,
        preserve_diacritic_strings: bool,
    ) -> crate::error::Result<Self> {
        let mut alpha = TransducerAlphabet {
            symbol_table: SymbolTable::new(),
            fd_table: FdTable::new(),
            unknown_symbol: NO_SYMBOL_NUMBER,
            default_symbol: NO_SYMBOL_NUMBER,
            identity_symbol: NO_SYMBOL_NUMBER,
            orig_symbol_count: 0,
            unicode_cache: Vec::new(),
        };
        let mut i: SymbolNumber = 0;
        while i < symbol_count {
            let mut str = read_symbol_string(is)?;
            if FdOperation::is_diacritic(&str) {
                alpha.fd_table.define_diacritic(i, &str);
                if !preserve_diacritic_strings {
                    str = String::new();
                }
            } else if is_unknown(&str) {
                alpha.unknown_symbol = i;
            } else if is_default(&str) {
                alpha.default_symbol = i;
            } else if is_identity(&str) {
                alpha.identity_symbol = i;
            }
            alpha.symbol_table.push(Symbol::from(str));
            i += 1;
        }
        alpha.orig_symbol_count = u32::try_from(alpha.symbol_table.len())
            .expect("value out of u32 range") as SymbolNumber;
        Ok(alpha)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.fake-read-alphabet-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.fake-read-alphabet-fn]
    pub fn fake_read_alphabet(is: &mut dyn std::io::BufRead, symbol_count: SymbolNumber) {
        let mut i: SymbolNumber = 0;
        while i < symbol_count {
            let _ = read_symbol_string(is);
            i += 1;
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.add-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.add-symbol-fn]
    pub fn add_symbol_str(&mut self, symbol: &str) {
        self.symbol_table.push(Symbol::new(symbol));
    }

    pub fn add_symbol(&mut self, symbol: &Symbol) {
        self.symbol_table.push(symbol.clone());
    }

    pub fn new_symboltable(st: &SymbolTable) -> Self {
        let mut alpha = TransducerAlphabet {
            symbol_table: st.clone(),
            fd_table: FdTable::new(),
            unknown_symbol: NO_SYMBOL_NUMBER,
            default_symbol: NO_SYMBOL_NUMBER,
            identity_symbol: NO_SYMBOL_NUMBER,
            orig_symbol_count: 0,
            unicode_cache: Vec::new(),
        };
        let mut i: SymbolNumber = 0;
        while (i as usize) < alpha.symbol_table.len() {
            if FdOperation::is_diacritic(&alpha.symbol_table[i as usize]) {
                let s = alpha.symbol_table[i as usize].clone();
                alpha.fd_table.define_diacritic(i, &s);
            } else if is_unknown(&alpha.symbol_table[i as usize]) {
                alpha.unknown_symbol = i;
            } else if is_default(&alpha.symbol_table[i as usize]) {
                alpha.default_symbol = i;
            } else if is_identity(&alpha.symbol_table[i as usize]) {
                alpha.identity_symbol = i;
            }
            i += 1;
        }
        alpha.orig_symbol_count = u32::try_from(alpha.symbol_table.len())
            .expect("value out of u32 range") as SymbolNumber;
        alpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.symbol-from-string-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.symbol-from-string-fn]
    pub fn symbol_from_string(&self, symbol_string: &str) -> Option<SymbolNumber> {
        for i in 0..self.symbol_table.len() {
            if self.symbol_table[i] == symbol_string {
                return Some(i as SymbolNumber);
            }
        }
        None
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.build-string-symbol-map-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.build-string-symbol-map-fn]
    pub fn build_string_symbol_map(&self) -> StringSymbolMap {
        let mut ss_map = StringSymbolMap::new();
        for i in 0..self.symbol_table.len() {
            ss_map.insert(self.symbol_table[i].clone(), i as SymbolNumber);
        }
        ss_map
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-like-epsilon-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-like-epsilon-fn]
    pub fn is_like_epsilon(&self, symbol: SymbolNumber) -> bool {
        if self.fd_table.is_diacritic(symbol) {
            return true;
        }
        if symbol as usize >= self.symbol_table.len() {
            return false;
        }
        let s = self.symbol_table[symbol as usize].as_bytes();
        // Check for Insert symbols like @I.something@ here
        if s.len() >= 5 && s[0] == b'@' && s[1] == b'I' && s[2] == b'.' && s[s.len() - 1] == b'@' {
            return true;
        }
        false
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-meta-arc-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-meta-arc-fn]
    #[inline]
    pub fn is_meta_arc(&self, symbol: SymbolNumber) -> bool {
        if symbol == NO_SYMBOL_NUMBER {
            return false;
        }
        (symbol == self.unknown_symbol)
            || (symbol == self.default_symbol)
            || (symbol == self.identity_symbol)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.cache-unicode-class-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.cache-unicode-class-fn]
    pub fn cache_unicode_class(&mut self, symbol: SymbolNumber) {
        while self.unicode_cache.len() <= symbol as usize {
            self.unicode_cache.push(UnicodeClassCacheValue::no_value);
        }
        if self.unicode_cache[symbol as usize] != UnicodeClassCacheValue::no_value {
            return;
        }
        // icu::UnicodeString::fromUTF8 + first code point's class. Rust 'char'
        // already carries the same Unicode properties ICU queries here.
        if let Some(c) = self.symbol_table[symbol as usize].chars().next() {
            if c.is_lowercase() {
                self.unicode_cache[symbol as usize] = UnicodeClassCacheValue::loweralpha;
            } else if c.is_uppercase() {
                self.unicode_cache[symbol as usize] = UnicodeClassCacheValue::upperalpha;
            } else if c.is_whitespace() {
                self.unicode_cache[symbol as usize] = UnicodeClassCacheValue::whitespace;
            } else {
                self.unicode_cache[symbol as usize] = UnicodeClassCacheValue::other;
            }
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-alpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-alpha-fn]
    pub fn is_unicode_alpha(&mut self, symbol: SymbolNumber) -> bool {
        self.cache_unicode_class(symbol);
        self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::loweralpha
            || self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::upperalpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-upperalpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-upperalpha-fn]
    pub fn is_unicode_upperalpha(&mut self, symbol: SymbolNumber) -> bool {
        self.cache_unicode_class(symbol);
        self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::upperalpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-loweralpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-loweralpha-fn]
    pub fn is_unicode_loweralpha(&mut self, symbol: SymbolNumber) -> bool {
        self.cache_unicode_class(symbol);
        self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::loweralpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-whitespace-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-whitespace-fn]
    pub fn is_unicode_whitespace(&mut self, symbol: SymbolNumber) -> bool {
        self.cache_unicode_class(symbol);
        self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::whitespace
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.display-fn]
    pub fn display(&self) {
        println!("Transducer alphabet:");
        for (i, sym) in self.symbol_table.iter().enumerate() {
            println!(" Symbol {}: {}", i, sym);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write) {
        for i in self.symbol_table.iter() {
            let _ = os.write_all(i.as_bytes());
            let _ = os.write_all(&[0u8]);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.has-flag-diacritics-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.has-flag-diacritics-fn]
    pub fn has_flag_diacritics(&self) -> bool {
        self.fd_table.num_features() > 0
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-flag-diacritic-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-flag-diacritic-fn]
    #[inline]
    pub fn is_flag_diacritic(&self, symbol: SymbolNumber) -> bool {
        self.fd_table.is_diacritic(symbol)
    }

    #[inline]
    pub fn get_symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.string-from-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.string-from-symbol-fn]
    // represent epsilon as blank string
    #[inline]
    pub fn string_from_symbol(&self, symbol: SymbolNumber) -> Symbol {
        if symbol == 0 {
            Symbol::default()
        } else {
            self.symbol_table[symbol as usize].clone()
        }
    }

    pub fn get_fd_table(&self) -> &FdTable<SymbolNumber> {
        &self.fd_table
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-operation-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-operation-fn]
    #[inline]
    pub fn get_operation(&self, symbol: SymbolNumber) -> Option<&FdOperation> {
        self.fd_table.get_operation(symbol)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-unknown-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-unknown-symbol-fn]
    #[inline]
    pub fn get_unknown_symbol(&self) -> SymbolNumber {
        self.unknown_symbol
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-default-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-default-symbol-fn]
    #[inline]
    pub fn get_default_symbol(&self) -> SymbolNumber {
        self.default_symbol
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-identity-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-identity-symbol-fn]
    #[inline]
    pub fn get_identity_symbol(&self) -> SymbolNumber {
        self.identity_symbol
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-orig-symbol-count-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-orig-symbol-count-fn]
    #[inline]
    pub fn get_orig_symbol_count(&self) -> SymbolNumber {
        self.orig_symbol_count
    }
}

impl Default for TransducerAlphabet {
    fn default() -> Self {
        Self::new()
    }
}
