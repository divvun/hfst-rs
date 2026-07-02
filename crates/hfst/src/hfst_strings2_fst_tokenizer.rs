//! Port of 'libhfst/src/HfstStrings2FstTokenizer.{h,cc}' — tokenizes
//! colon/space/backslash-escaped pair strings into 'StringPairVector's, built
//! on top of 'HfstTokenizer'.

use crate::hfst_data_types::StringPair;
use crate::hfst_tokenizer::HfstTokenizer;

// [spec:hfst:def:hfst-strings2-fst-tokenizer.string-vector]
pub type StringVector = Vec<String>;
// [spec:hfst:def:hfst-strings2-fst-tokenizer.string-pair]
// (StringPair = (String, String), shared with hfst_data_types)
// [spec:hfst:def:hfst-strings2-fst-tokenizer.string-pair-vector]
pub type StringPairVector = Vec<StringPair>;

const COL: &str = ":";
const BACKSLASH: &str = "\\";
const SPACE: &str = " ";
const BACKSLASH_ESC: &str = "@_BACKSLASH_@";
const EPSILON_SYMBOL: &str = "@_EPSILON_SYMBOL_@";
const EMPTY: &str = "";

const COL_CHAR: u8 = b':';
const BACKSLASH_CHAR: u8 = b'\\';

const COL_ESCAPE: &str = "@_COLON_@";
const TAB_ESCAPE: &str = "@_TAB_@";
const SPACE_ESCAPE: &str = "@_SPACE_@";

// [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer]
pub struct HfstStrings2FstTokenizer {
    tokenizer: HfstTokenizer,
    eps: String,
}

#[allow(dead_code)]
impl HfstStrings2FstTokenizer {
    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.hfst-strings2-fst-tokenizer-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.hfst-strings2-fst-tokenizer-fn]
    pub fn new(multichar_symbols: &StringVector, eps: &str) -> crate::error::Result<Self> {
        let mut t = HfstStrings2FstTokenizer {
            tokenizer: HfstTokenizer::new(),
            eps: eps.to_string(),
        };
        // \: \\ \<space> and eps are special cases.
        if !eps.is_empty() {
            t.add_multichar_symbol(eps);
        }

        t.tokenizer.add_multichar_symbol("\\:"); // BACKSLASH COL
        t.tokenizer.add_multichar_symbol("\\ "); // BACKSLASH SPACE
        t.tokenizer.add_multichar_symbol("\\\\"); // BACKSLASH BACKSLASH
        t.add_multichar_symbol(COL_ESCAPE);
        t.add_multichar_symbol(TAB_ESCAPE);
        t.add_multichar_symbol(SPACE_ESCAPE);

        if !eps.is_empty() {
            t.tokenizer.add_multichar_symbol(eps);
            t.add_multichar_symbol_head(eps)?;
        }
        t.add_multichar_symbol_head(SPACE_ESCAPE)?;

        for it in multichar_symbols.iter() {
            t.add_multichar_symbol_head(it)?;
            t.add_multichar_symbol(it);
        }
        Ok(t)
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.add-multichar-symbol-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.add-multichar-symbol-fn]
    pub fn add_multichar_symbol(&mut self, multichar_symbol: &str) {
        self.tokenizer.add_multichar_symbol(multichar_symbol);
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.add-multichar-symbol-head-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.add-multichar-symbol-head-fn]
    pub fn add_multichar_symbol_head(
        &mut self,
        multichar_symbol: &str,
    ) -> crate::error::Result<()> {
        if multichar_symbol.is_empty() {
            crate::bail!(EmptyMulticharSymbol);
        }
        let tokenized_multichar_symbol = self.tokenizer.tokenize_one_level(multichar_symbol, false);
        let multichar_symbol_head = tokenized_multichar_symbol[0].clone();
        self.tokenizer
            .add_multichar_symbol(&(BACKSLASH.to_string() + &multichar_symbol_head));
        Ok(())
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.tokenize-pair-string-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.tokenize-pair-string-fn]
    pub fn tokenize_pair_string(
        &self,
        str: &str,
        spaces: bool,
    ) -> crate::error::Result<StringPairVector> {
        let tokenized_str = if spaces {
            self.split_at_spaces(str)
        } else {
            let mut t = self.tokenizer.tokenize_one_level(str, false);
            // std::remove(.., BACKSLASH) + erase
            t.retain(|s| s != BACKSLASH);
            t
        };
        self.make_pair_vector_v(&tokenized_str)
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.tokenize-string-pair-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.tokenize-string-pair-fn]
    pub fn tokenize_string_pair(
        &self,
        str: &str,
        spaces: bool,
    ) -> crate::error::Result<StringPairVector> {
        let tokenized_str = if spaces {
            self.split_at_spaces(str)
        } else {
            self.tokenizer.tokenize_one_level(str, false)
        };
        match tokenized_str.iter().position(|s| s == COL) {
            None => self.make_pair_vector_io(&tokenized_str, &tokenized_str),
            Some(it) => {
                let left: StringVector = tokenized_str[..it].to_vec();
                let right: StringVector = tokenized_str[it + 1..].to_vec();
                self.make_pair_vector_io(&left, &right)
            }
        }
    }

    fn make_pair_vector_v(&self, v: &StringVector) -> crate::error::Result<StringPairVector> {
        let mut spv = StringPairVector::new();
        let mut i = 0;
        while i < v.len() {
            if !self.is_pair_input_symbol(v, i) {
                let mut symbol = self.unescape(v[i].clone())?;
                symbol = if symbol.is_empty() || symbol == self.eps {
                    EPSILON_SYMBOL.to_string()
                } else {
                    symbol
                };
                spv.push((symbol.clone(), symbol));
            } else {
                let input = if v[i].is_empty() || v[i] == self.eps {
                    EPSILON_SYMBOL.to_string()
                } else {
                    self.unescape(v[i].clone())?
                };
                i += 2; // ++(++it)
                let output = if v[i].is_empty() || v[i] == self.eps {
                    EPSILON_SYMBOL.to_string()
                } else {
                    self.unescape(v[i].clone())?
                };
                spv.push((input, output));
            }
            i += 1;
        }
        Ok(spv)
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.make-pair-vector-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.make-pair-vector-fn]
    fn make_pair_vector_io(
        &self,
        input: &StringVector,
        output: &StringVector,
    ) -> crate::error::Result<StringPairVector> {
        let mut spv = StringPairVector::new();
        let mut input_it = 0;
        let mut output_it = 0;
        while input_it < input.len() && output_it < output.len() {
            let input_symbol = self.unescape(input[input_it].clone())?;
            let output_symbol = self.unescape(output[output_it].clone())?;

            spv.push((
                if input_symbol.is_empty() || input_symbol == self.eps {
                    EPSILON_SYMBOL.to_string()
                } else {
                    input_symbol
                },
                if output_symbol.is_empty() || output_symbol == self.eps {
                    EPSILON_SYMBOL.to_string()
                } else {
                    output_symbol
                },
            ));
            input_it += 1;
            output_it += 1;
        }
        if input_it == input.len() {
            while output_it < output.len() {
                spv.push((
                    EPSILON_SYMBOL.to_string(),
                    if output[output_it].is_empty() || output[output_it] == self.eps {
                        EPSILON_SYMBOL.to_string()
                    } else {
                        self.unescape(output[output_it].clone())?
                    },
                ));
                output_it += 1;
            }
        } else {
            while input_it < input.len() {
                spv.push((
                    if input[input_it].is_empty() || input[input_it] == self.eps {
                        EPSILON_SYMBOL.to_string()
                    } else {
                        self.unescape(input[input_it].clone())?
                    },
                    EPSILON_SYMBOL.to_string(),
                ));
                input_it += 1;
            }
        }
        Ok(spv)
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.unescape-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.unescape-fn]
    fn unescape(&self, mut symbol: String) -> crate::error::Result<String> {
        self.check_cols(&symbol)?;

        if symbol == "\\\\" {
            // BACKSLASH BACKSLASH
            return Ok(BACKSLASH.to_string());
        }

        while let Some(pos) = symbol.find("\\\\") {
            symbol.replace_range(pos..pos + 2, BACKSLASH_ESC);
        }

        while let Some(pos) = symbol.find(BACKSLASH) {
            symbol.replace_range(pos..pos + 1, EMPTY);
        }

        while let Some(pos) = symbol.find(BACKSLASH_ESC) {
            symbol.replace_range(pos..pos + BACKSLASH_ESC.len(), EMPTY);
        }

        while let Some(pos) = symbol.find(SPACE_ESCAPE) {
            symbol.replace_range(pos..pos + SPACE_ESCAPE.len(), " ");
        }

        while let Some(pos) = symbol.find(TAB_ESCAPE) {
            symbol.replace_range(pos..pos + TAB_ESCAPE.len(), "   ");
        }

        while let Some(pos) = symbol.find(COL_ESCAPE) {
            symbol.replace_range(pos..pos + COL_ESCAPE.len(), ":");
        }

        Ok(symbol)
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.is-pair-input-symbol-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.is-pair-input-symbol-fn]
    fn is_pair_input_symbol(&self, v: &[String], i: usize) -> bool {
        // C++ walks an iterator from 'it': current, then next must be COL, then
        // another must follow.
        if i >= v.len() {
            return false;
        }
        if i + 1 >= v.len() {
            return false;
        }
        if v[i + 1] != COL {
            return false;
        }
        if i + 2 >= v.len() {
            return false;
        }
        true
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.check-cols-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.check-cols-fn]
    fn check_cols(&self, symbol: &str) -> crate::error::Result<()> {
        let bytes = symbol.as_bytes();
        if !symbol.is_empty() {
            if bytes[0] == COL_CHAR {
                crate::bail!(UnescapedColsFound);
            }
            let mut pos = 0usize;
            loop {
                // symbol.find(COL_CHAR, pos + 1)
                match bytes[pos + 1..].iter().position(|&b| b == COL_CHAR) {
                    None => break,
                    Some(off) => {
                        pos = pos + 1 + off;
                        if bytes[pos - 1] != BACKSLASH_CHAR {
                            crate::bail!(UnescapedColsFound);
                        }
                        if pos > 1 && bytes[pos - 2] == BACKSLASH_CHAR {
                            crate::bail!(UnescapedColsFound);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.get-col-pos-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.get-col-pos-fn]
    fn get_col_pos(&self, str: &str) -> i32 {
        let bytes = str.as_bytes();
        if str.is_empty() {
            return -1;
        }
        if bytes[0] == COL_CHAR {
            return 0;
        }
        for i in 1..bytes.len() {
            if bytes[i] == COL_CHAR && bytes[i - 1] != BACKSLASH_CHAR {
                return i as i32;
            }
        }
        -1
    }

    // [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.split-at-spaces-fn]
    // [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.split-at-spaces-fn]
    fn split_at_spaces(&self, str: &str) -> StringVector {
        let mut symbol = String::new();
        let sv = self.tokenizer.tokenize_one_level(str, false);
        let mut res = StringVector::new();
        let mut i = 0;
        while i < sv.len() {
            let it = sv[i].clone();
            if it == SPACE && !symbol.is_empty() {
                res.push(symbol.clone());
                while i + 1 < sv.len() && sv[i + 1] == SPACE {
                    i += 1;
                }
                symbol = EMPTY.to_string();
                if i >= sv.len() {
                    break;
                }
            } else if it == SPACE {
                while i + 1 < sv.len() && sv[i + 1] == SPACE {
                    i += 1;
                }
            } else if it == COL && !symbol.is_empty() {
                res.push(symbol.clone());
                res.push(COL.to_string());
                symbol = EMPTY.to_string();
            } else if it == COL {
                res.push(COL.to_string());
            } else {
                symbol += &it;
            }
            i += 1;
        }
        if !symbol.is_empty() {
            res.push(symbol);
        }
        res
    }
}
