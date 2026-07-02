//! Port of 'libhfst/src/HfstTokenizer.{h,cc}' — a tokenizer for creating
//! transducers from UTF-8 strings (longest-match multichar tokenization).
//!
//! The C++ used ICU4C ('ubrk'/'u_strFromUTF8'); here:
//! - grapheme-cluster segmentation uses the 'icu' crate
//!   ('GraphemeClusterSegmenter'), constructed per call as the C++ opened a new
//!   'UBreakIterator' per call;
//! - UTF-8 *validation* collapses to a no-op, because a Rust '&str' is valid
//!   UTF-8 by construction (so 'check_utf8_correctness' never throws and the
//!   "split a single UTF-8 char" path is just the next 'char''s byte length).
//!
//! 'MultiCharSymbolTrie' keeps the C++ algorithm verbatim, walking the input by
//! byte index (mirroring the 'const char*' pointer arithmetic, with "past end"
//! reading as a NUL byte); child tries are owned 'Box'es (the faithful owning
//! equivalent of the C++ 'new'/'delete' pointers, auto-freed where the C++
//! destructor 'delete'd them).

use icu::segmenter::GraphemeClusterSegmenter;

use crate::hfst_data_types::StringVector;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::{StringPair, StringPairVector, StringSet, internal_epsilon};

// 'UCHAR_MAX' (8-bit). The C++ sizes the child/leaf vectors at exactly this; a
// byte value of 255 would be out of bounds (UB there, a panic here), but valid
// UTF-8 never produces 0xFF, so it cannot occur via the '&str' API.
const UCHAR_MAX: usize = 255;
// 'std::string::npos'.
const NPOS: usize = usize::MAX;

// '*p' where 'p' walks a NUL-terminated buffer: byte at 'i', or 0 past the end.
fn byte_at(p: &[u8], i: usize) -> u8 {
    if i < p.len() { p[i] } else { 0 }
}

// [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie]
// The C++ destructor 'delete's every child trie; here the owned 'Box'es are
// dropped automatically.
// [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.multi-char-symbol-trie-fn]
// [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.multi-char-symbol-trie-fn]
pub struct MultiCharSymbolTrie {
    symbol_rests: Vec<Option<Box<MultiCharSymbolTrie>>>,
    is_leaf: Vec<bool>,
}

impl MultiCharSymbolTrie {
    // [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.is-end-of-string-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.is-end-of-string-fn]
    fn is_end_of_string(p: &[u8], pos: usize) -> bool {
        byte_at(p, pos + 1) == 0
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.set-symbol-end-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.set-symbol-end-fn]
    fn set_symbol_end(&mut self, p: &[u8], pos: usize) {
        self.is_leaf[byte_at(p, pos) as usize] = true;
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.is-symbol-end-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.is-symbol-end-fn]
    fn is_symbol_end(&self, p: &[u8], pos: usize) -> bool {
        self.is_leaf[byte_at(p, pos) as usize]
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.init-symbol-rests-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.init-symbol-rests-fn]
    fn init_symbol_rests(&mut self, p: &[u8], pos: usize) {
        let idx = byte_at(p, pos) as usize;
        if self.symbol_rests[idx].is_none() {
            self.symbol_rests[idx] = Some(Box::new(MultiCharSymbolTrie::new()));
        }
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.add-symbol-rest-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.add-symbol-rest-fn]
    fn add_symbol_rest(&mut self, p: &[u8], pos: usize) {
        let idx = byte_at(p, pos) as usize;
        self.symbol_rests[idx].as_mut().unwrap().add(p, pos + 1);
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.get-symbol-rest-trie-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.get-symbol-rest-trie-fn]
    fn get_symbol_rest_trie(&self, p: &[u8], pos: usize) -> Option<&MultiCharSymbolTrie> {
        self.symbol_rests[byte_at(p, pos) as usize].as_deref()
    }

    pub fn new() -> Self {
        MultiCharSymbolTrie {
            symbol_rests: (0..UCHAR_MAX).map(|_| None).collect(),
            is_leaf: vec![false; UCHAR_MAX],
        }
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.add-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.add-fn]
    pub fn add(&mut self, p: &[u8], pos: usize) {
        if Self::is_end_of_string(p, pos) {
            self.set_symbol_end(p, pos);
        } else {
            self.init_symbol_rests(p, pos);
            self.add_symbol_rest(p, pos);
        }
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.find-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.find-fn]
    //
    // Returns the absolute byte index past the matched symbol (the C++ 'p+1'
    // pointer expressed as an offset), or 'None' for the C++ 'NULL'.
    pub fn find(&self, p: &[u8], pos: usize) -> Option<usize> {
        let symbol_rest_trie = self.get_symbol_rest_trie(p, pos);
        match symbol_rest_trie {
            None => {
                if self.is_symbol_end(p, pos) {
                    return Some(pos + 1);
                }
                None
            }
            Some(symbol_rest_trie) => {
                let symbol_end = symbol_rest_trie.find(p, pos + 1);
                match symbol_end {
                    None => {
                        if self.is_symbol_end(p, pos) {
                            return Some(pos + 1);
                        }
                        None
                    }
                    Some(symbol_end) => Some(symbol_end),
                }
            }
        }
    }
}

impl Default for MultiCharSymbolTrie {
    fn default() -> Self {
        Self::new()
    }
}

/// \brief A tokenizer for creating transducers from UTF-8 strings, using
/// longest-match tokenization.
// [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer]
pub struct HfstTokenizer {
    multi_char_symbols: MultiCharSymbolTrie,
    skip_symbol_set: StringSet,
}

impl HfstTokenizer {
    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.hfst-tokenizer-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.hfst-tokenizer-fn]
    pub fn new() -> Self {
        HfstTokenizer {
            multi_char_symbols: MultiCharSymbolTrie::new(),
            skip_symbol_set: StringSet::new(),
        }
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.get-next-symbol-size-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.get-next-symbol-size-fn]
    fn get_next_symbol_size(&self, symbol: &str, split_characters: bool) -> i32 {
        if symbol.is_empty() {
            return 0;
        }

        let multi_char_symbol_end = self.multi_char_symbols.find(symbol.as_bytes(), 0);

        /* The string begins with a multi character symbol */
        if let Some(multi_char_symbol_end) = multi_char_symbol_end {
            return multi_char_symbol_end as i32;
        }
        /* take next combining grapheme cluster */
        if !split_characters {
            let segmenter = GraphemeClusterSegmenter::new();
            let mut bounds = segmenter.segment_str(symbol);
            let begin = bounds.next().unwrap_or(0);
            let end = bounds.next();
            match end {
                // UBRK_DONE
                None => 0,
                Some(end) => {
                    if begin == end {
                        0
                    } else {
                        (end - begin) as i32 // number of bytes
                    }
                }
            }
        }
        /* split_characters => take next UTF-8 only */
        else {
            symbol.chars().next().unwrap().len_utf8() as i32
        }
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.is-skip-symbol-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.is-skip-symbol-fn]
    fn is_skip_symbol(&self, s: &str) -> bool {
        s.is_empty() || self.skip_symbol_set.contains(s)
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.add-multichar-symbol-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.add-multichar-symbol-fn]
    pub fn add_multichar_symbol(&mut self, symbol: &str) {
        if symbol.is_empty() {
            return;
        }
        self.multi_char_symbols.add(symbol.as_bytes(), 0);
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.add-skip-symbol-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.add-skip-symbol-fn]
    pub fn add_skip_symbol(&mut self, symbol: &str) {
        if symbol.is_empty() {
            return;
        }
        self.multi_char_symbols.add(symbol.as_bytes(), 0);
        self.skip_symbol_set.insert(symbol.to_string());
    }

    pub fn tokenize(&self, input_string: &str, split_characters: bool) -> StringPairVector {
        Self::check_utf8_correctness(input_string);
        let mut spv = StringPairVector::new();
        let bytes = input_string.as_bytes();
        let mut s: usize = 0;
        while s < bytes.len() {
            let symbol_size = self.get_next_symbol_size(&input_string[s..], split_characters);
            let symbol = input_string[s..s + symbol_size as usize].to_string();
            s += symbol_size as usize;
            if self.is_skip_symbol(&symbol) {
                continue;
            }
            spv.push((symbol.clone(), symbol));
        }
        spv
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-one-level-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-one-level-fn]
    pub fn tokenize_one_level(&self, input_string: &str, split_characters: bool) -> StringVector {
        Self::check_utf8_correctness(input_string);

        let mut sv = StringVector::new();
        let bytes = input_string.as_bytes();
        let mut s: usize = 0;
        while s < bytes.len() {
            let symbol_size = self.get_next_symbol_size(&input_string[s..], split_characters);
            let symbol = input_string[s..s + symbol_size as usize].to_string();
            s += symbol_size as usize;
            if self.is_skip_symbol(&symbol) {
                continue;
            }
            sv.push(symbol);
        }
        sv
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-space-separated-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-space-separated-fn]
    pub fn tokenize_space_separated(str: &str) -> StringPairVector {
        Self::check_utf8_correctness(str);

        let mut retval = StringPairVector::new();
        let bytes = str.as_bytes();
        let mut pos: usize = 0;
        // position where a symbol begins, not yet defined
        let mut symbol_pos: usize = NPOS;

        while pos < str.len() {
            // end of symbol reached
            if bytes[pos] == b' ' && symbol_pos != NPOS {
                let symbol = str[symbol_pos..pos].to_string();
                retval.push((symbol.clone(), symbol));
                symbol_pos = NPOS; // next symbol not yet found
            }
            // next symbol found
            else if bytes[pos] != b' ' && symbol_pos == NPOS {
                symbol_pos = pos;
            } else {
            }
            pos += 1;
        }

        // last symbol
        if symbol_pos != NPOS {
            let symbol = str[symbol_pos..].to_string();
            retval.push((symbol.clone(), symbol));
        }

        retval
    }

    pub fn tokenize_pair(
        &self,
        input_string: &str,
        output_string: &str,
        split_characters: bool,
    ) -> StringPairVector {
        Self::check_utf8_correctness(input_string);
        Self::check_utf8_correctness(output_string);

        let mut spv = StringPairVector::new();

        let input_spv = self.tokenize(input_string, split_characters);
        let output_spv = self.tokenize(output_string, split_characters);

        if input_spv.len() < output_spv.len() {
            let mut jt = 0;
            for it in input_spv.iter() {
                spv.push((it.0.clone(), output_spv[jt].0.clone()));
                jt += 1;
            }
            for k in jt..output_spv.len() {
                spv.push((internal_epsilon.to_string(), output_spv[k].0.clone()));
            }
        } else {
            let mut it = 0;
            for jt in output_spv.iter() {
                spv.push((input_spv[it].0.clone(), jt.0.clone()));
                it += 1;
            }
            for k in it..input_spv.len() {
                spv.push((input_spv[k].0.clone(), internal_epsilon.to_string()));
            }
        }
        spv
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-fn]
    pub fn tokenize_pair_warn(
        &self,
        input_string: &str,
        output_string: &str,
        split_characters: bool,
        warn_about_pair: fn(&StringPair),
    ) -> StringPairVector {
        Self::check_utf8_correctness(input_string);
        Self::check_utf8_correctness(output_string);

        let mut spv = StringPairVector::new();

        let input_spv = self.tokenize(input_string, split_characters);
        let output_spv = self.tokenize(output_string, split_characters);

        if input_spv.len() < output_spv.len() {
            let mut jt = 0;
            for it in input_spv.iter() {
                let sp = (it.0.clone(), output_spv[jt].0.clone());
                warn_about_pair(&sp);
                spv.push(sp);
                jt += 1;
            }
            for k in jt..output_spv.len() {
                let sp = (internal_epsilon.to_string(), output_spv[k].0.clone());
                warn_about_pair(&sp);
                spv.push(sp);
            }
        } else {
            let mut it = 0;
            for jt in output_spv.iter() {
                let sp = (input_spv[it].0.clone(), jt.0.clone());
                warn_about_pair(&sp);
                spv.push(sp);
                it += 1;
            }
            for k in it..input_spv.len() {
                let sp = (input_spv[k].0.clone(), internal_epsilon.to_string());
                warn_about_pair(&sp);
                spv.push(sp);
            }
        }
        spv
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-and-align-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-and-align-flag-diacritics-fn]
    pub fn tokenize_and_align_flag_diacritics(
        &self,
        input_string: &str,
        output_string: &str,
        split_characters: bool,
        warn_about_pair: fn(&StringPair),
    ) -> StringPairVector {
        Self::check_utf8_correctness(input_string);
        Self::check_utf8_correctness(output_string);

        let mut spv = StringPairVector::new();

        let input_spv = self.tokenize(input_string, split_characters);
        let output_spv = self.tokenize(output_string, split_characters);

        assert!(input_spv.len() > 0 && output_spv.len() > 0);
        let mut it = 0;
        let mut jt = 0;

        // proceed until both token vectors are exhausted
        while it != input_spv.len() || jt != output_spv.len() {
            // string pair to push back to the result (assigned in every branch)
            let sp: StringPair;
            // possible continuation in case of missaligned flags
            let mut sp_cont: StringPair = (String::new(), String::new());

            if it == input_spv.len() {
                if FdOperation::is_diacritic(&output_spv[jt].0) {
                    // copy diacritic to other side
                    sp = (output_spv[jt].0.clone(), output_spv[jt].0.clone());
                } else {
                    // pad input with epsilons
                    sp = (internal_epsilon.to_string(), output_spv[jt].0.clone());
                }
                jt += 1;
            } else if jt == output_spv.len() {
                if FdOperation::is_diacritic(&input_spv[it].0) {
                    // copy diacritic to other side
                    sp = (input_spv[it].0.clone(), input_spv[it].0.clone());
                } else {
                    // pad output with epsilons
                    sp = (input_spv[it].0.clone(), internal_epsilon.to_string());
                }
                it += 1;
            } else {
                // take from both vectors (cases foo:bar, foo:foo, flag1:flag1)
                if (!FdOperation::is_diacritic(&input_spv[it].0)
                    && !FdOperation::is_diacritic(&output_spv[jt].0))
                    || input_spv[it] == output_spv[jt]
                {
                    sp = (input_spv[it].0.clone(), output_spv[jt].0.clone());
                }
                // take first from first vector and then from second
                // (cases flag1:flag2, flag1::bar, foo:flag2)
                else {
                    let wrong_pair = (input_spv[it].0.clone(), output_spv[jt].0.clone());
                    warn_about_pair(&wrong_pair);
                    sp = (input_spv[it].0.clone(), input_spv[it].0.clone());
                    sp_cont = (output_spv[jt].0.clone(), output_spv[jt].0.clone());
                }
                it += 1;
                jt += 1;
            }

            spv.push(sp);
            if sp_cont.0.len() != 0 && sp_cont.1.len() != 0 {
                spv.push(sp_cont);
            }
        }

        spv
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-fn]
    pub fn check_utf8_correctness(input_string: &str) {
        let _ = Self::check_utf8_correctness_and_calculate_length(input_string);
    }

    // [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-and-calculate-length-fn]
    // [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-and-calculate-length-fn]
    //
    // A Rust '&str' is always valid UTF-8, so the original ICU validity check
    // can never fail (no 'IncorrectUtf8CodingException'). The return value is the
    // UTF-16 code-unit length, as 'u_strFromUTF8' measured.
    pub fn check_utf8_correctness_and_calculate_length(input_string: &str) -> u32 {
        input_string.encode_utf16().count() as u32
    }
}

impl Default for HfstTokenizer {
    fn default() -> Self {
        Self::new()
    }
}
