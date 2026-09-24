//! Input tokenization: the letter trie and the encoder built over it.

use super::*;

// [spec:hfst:def:transducer.hfst-ol.ol-letter-trie-vector]
pub type OlLetterTrieVector = Vec<Option<Box<OlLetterTrie>>>;

// [spec:hfst:def:transducer.hfst-ol.ol-letter-trie]
pub struct OlLetterTrie {
    letters: OlLetterTrieVector,
    symbols: SymbolNumberVector,
}

impl OlLetterTrie {
    pub fn new() -> Self {
        let mut letters: OlLetterTrieVector = Vec::with_capacity(u8::MAX as usize + 1);
        for _ in 0..(u8::MAX as usize + 1) {
            letters.push(None);
        }
        OlLetterTrie {
            letters,
            symbols: vec![NO_SYMBOL_NUMBER; u8::MAX as usize + 1],
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.add-string-fn]
    // [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.add-string-fn]
    // 'p' is a 0-terminated byte slice positioned at the current char.
    pub fn add_string(&mut self, p: &[u8], symbol_key: SymbolNumber) {
        if p[1] == 0 {
            self.symbols[p[0] as usize] = symbol_key;
            return;
        }
        if self.letters[p[0] as usize].is_none() {
            self.letters[p[0] as usize] = Some(Box::new(OlLetterTrie::new()));
        }
        let idx = p[0] as usize;
        self.letters[idx]
            .as_mut()
            .expect("letters entry was set to Some just above")
            .add_string(&p[1..], symbol_key);
    }

    // [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.has-key-starting-with-fn]
    // [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.has-key-starting-with-fn]
    pub fn has_key_starting_with(&self, c: u8) -> bool {
        self.letters[c as usize].is_some()
    }

    // [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.find-key-fn]
    // [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.find-key-fn]
    // 'p' is an index into 'buf' advanced by reference, mirroring 'char ** p'.
    // 'None' is the C++ NO_SYMBOL_NUMBER "no tokenization found" result.
    pub fn find_key(&self, buf: &[u8], p: &mut usize) -> Option<SymbolNumber> {
        // The leaf table stores NO_SYMBOL_NUMBER for unmapped bytes.
        fn leaf(sym: SymbolNumber) -> Option<SymbolNumber> {
            (sym != NO_SYMBOL_NUMBER).then_some(sym)
        }
        let old_p = *p;
        *p += 1;
        match self.letters[buf[old_p] as usize].as_ref() {
            None => leaf(self.symbols[buf[old_p] as usize]),
            Some(child) => match child.find_key(buf, p) {
                Some(s) => Some(s),
                None => {
                    *p -= 1;
                    leaf(self.symbols[buf[old_p] as usize])
                }
            },
        }
    }
}

impl Drop for OlLetterTrie {
    // [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.ol-letter-trie-fn]
    // [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.ol-letter-trie-fn]
    fn drop(&mut self) {
        for i in 0..self.letters.len() {
            // 'delete letters[i]; letters[i] = 0;' — dropping the 'Box' frees the
            // child trie (which recursively frees its children) and resetting the
            // slot to 'None' mirrors the null assignment.
            self.letters[i] = None;
        }
    }
}

impl Default for OlLetterTrie {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:transducer.hfst-ol.encoder]
pub struct Encoder {
    number_of_input_symbols: SymbolNumber,
    letters: OlLetterTrie,
    ascii_symbols: SymbolNumberVector,
}

impl Encoder {
    // [spec:hfst:def:transducer.hfst-ol.encoder.encoder-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.encoder-fn]
    pub fn new(st: &SymbolTable, input_symbol_count: SymbolNumber) -> Self {
        let mut encoder = Encoder {
            number_of_input_symbols: input_symbol_count,
            letters: OlLetterTrie::new(),
            ascii_symbols: vec![NO_SYMBOL_NUMBER; 128],
        };
        encoder.read_input_symbols(st);
        encoder
    }

    // [spec:hfst:def:transducer.hfst-ol.encoder.read-input-symbols-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.read-input-symbols-fn]
    //
    // Two passes. Pass 1 indexes every symbol under its OWN, verbatim spelling —
    // that is the C++ walk, and it must win outright. Pass 2 adds the [#439]
    // normalization aliases, and only for spellings pass 1 left unclaimed.
    //
    // The order matters because an alias can collide with a real symbol: U+0387
    // GREEK ANO TELEIA has a singleton canonical decomposition to U+00B7 MIDDLE
    // DOT, so an alphabet containing BOTH (the Giella tokenisers do) had the
    // U+0387 alias overwrite the genuine U+00B7 entry whenever U+0387 came later
    // in the symbol table. Input U+00B7 then encoded to the U+0387 symbol: the
    // U+00B7 analyses vanished and even the echoed surface form was corrupted.
    // Registering aliases only into free slots keeps [#439] (an alias spelling
    // that is not itself a symbol still resolves) without ever shadowing a real
    // symbol.
    pub fn read_input_symbols(&mut self, kt: &SymbolTable) {
        for k in 0..self.number_of_input_symbols {
            let sym = kt[k as usize].clone();
            self.read_input_symbol_form(&sym, k as i32);
        }
        for k in 0..self.number_of_input_symbols {
            let sym = kt[k as usize].clone();
            self.read_alias_forms(&sym, k as i32);
        }
    }

    // True when the encoder already tokenizes exactly `s` to some symbol, so the
    // spelling is spoken for and an alias must not overwrite it. Runs the real
    // 'find_key' and demands it consume the whole string, so a prefix match (a
    // one-byte ascii symbol at the head of a longer spelling, say) is not
    // mistaken for a claim on the longer one.
    fn spelling_is_taken(&self, s: &str) -> bool {
        if s.is_empty() {
            return true;
        }
        // 'find_key' walks a 0-terminated buffer; the terminator is what stops
        // the trie descent reading past the end when the last byte has children.
        let mut buf = s.as_bytes().to_vec();
        buf.push(0);
        let mut p = 0usize;
        self.find_key(&buf, &mut p).is_some() && p == s.len()
    }

    // Register `s`'s normalization aliases against `s_num`, skipping any
    // spelling already claimed — by a real symbol or by an earlier alias.
    fn read_alias_forms(&mut self, s: &str, s_num: i32) {
        for form in Self::normalization_aliases(s) {
            if !self.spelling_is_taken(&form) {
                self.read_input_symbol_form(&form, s_num);
            }
        }
    }

    // [#439] Grapheme cluster is the port's logical tokenization unit, so a base
    // + combining diacritic and its precomposed form are the SAME unit and must
    // match the same symbol. These are the alternative spellings a symbol is
    // additionally indexed under (when they differ from it and from each other),
    // all mapping to the same symbol number, so input in either normalization
    // tokenizes to this symbol. Output is unaffected — the number still maps
    // back to the original surface.
    fn normalization_aliases(s: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let nfc = icu::normalizer::ComposingNormalizerBorrowed::new_nfc().normalize(s);
        if nfc.as_ref() != s {
            out.push(nfc.to_string());
        }
        let nfd = icu::normalizer::DecomposingNormalizerBorrowed::new_nfd().normalize(s);
        if nfd.as_ref() != s && nfd != nfc {
            out.push(nfd.into_owned());
        }
        out
    }

    // [spec:hfst:def:transducer.hfst-ol.encoder.read-input-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.read-input-symbol-fn]
    // Single-symbol registration, for the incremental callers that add a symbol
    // to an already-built encoder. The symbol's own spelling is registered
    // unconditionally (a real symbol always wins); its aliases only claim
    // spellings nothing else has, on the same rule as the table-wide walk.
    pub fn read_input_symbol(&mut self, s: &str, s_num: i32) {
        self.read_input_symbol_form(s, s_num);
        self.read_alias_forms(s, s_num);
    }

    fn read_input_symbol_form(&mut self, s: &str, s_num: i32) {
        let bytes = s.as_bytes();
        let strlen = bytes.len();
        // A symbol that spells the empty string cannot be tokenized out of the
        // input — it would consume no bytes — and the trie walk below assumes at
        // least one byte before the terminator. An alphabet read from a stream
        // can hold one (two adjacent NUL separators), so drop it here rather
        // than indexing past the end of a one-byte buffer.
        if strlen == 0 {
            return;
        }
        if strlen == 1
            && should_ascii_tokenize(bytes[0])
            && !self.letters.has_key_starting_with(bytes[0])
        {
            self.ascii_symbols[bytes[0] as usize] = s_num as SymbolNumber;
        }
        // If there's an ascii tokenized symbol shadowing this, remove it
        if strlen > 1
            && should_ascii_tokenize(bytes[0])
            && self.ascii_symbols[bytes[0] as usize] != NO_SYMBOL_NUMBER
        {
            self.ascii_symbols[bytes[0] as usize] = NO_SYMBOL_NUMBER;
        }
        // add_string walks a 0-terminated buffer.
        let mut buf = bytes.to_vec();
        buf.push(0);
        self.letters.add_string(&buf, s_num as SymbolNumber);
    }

    // [spec:hfst:def:transducer.hfst-ol.encoder.find-key-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.find-key-fn]
    pub fn find_key(&self, buf: &[u8], p: &mut usize) -> Option<SymbolNumber> {
        if !should_ascii_tokenize(buf[*p])
            || self.ascii_symbols[buf[*p] as usize] == NO_SYMBOL_NUMBER
        {
            return self.letters.find_key(buf, p);
        }
        let s = self.ascii_symbols[buf[*p] as usize];
        *p += 1;
        Some(s)
    }
}

// [spec:hfst:def:transducer.hfst-ol.should-ascii-tokenize-fn]
// [spec:hfst:sem:transducer.hfst-ol.should-ascii-tokenize-fn]
pub fn should_ascii_tokenize(c: u8) -> bool {
    c <= 127
}
