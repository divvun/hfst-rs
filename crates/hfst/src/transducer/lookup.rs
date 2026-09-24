//! Tokenization checks and the one-shot lookup surface.

use super::*;
use crate::hfst_data_types::{HfstOneLevelPaths, HfstTwoLevelPaths, StringVector};
use crate::lookup_state::LookupState;

impl<T: TransducerTablesInterface> Transducer<T> {
    /// Whether `input` tokenizes wholly into symbols the alphabet already has.
    ///
    /// [`Self::initialize_input`] answers a different question: it ADOPTS an
    /// unrecognized symbol into the alphabet and carries on, so it fails only on
    /// malformed UTF-8. The C++ `find_next_key` loop in hfst-optimized-lookup is
    /// the strict reading, and the tool distinguishes the two outcomes — a word
    /// it cannot tokenize is reported unanalysable even in fast mode, while a
    /// word that tokenizes and simply has no analysis is not.
    pub fn can_tokenize(&self, input: &str) -> bool {
        let mut buf: Vec<u8> = input.as_bytes().to_vec();
        buf.push(0);
        let mut p: usize = 0;
        let encoder = self
            .encoder
            .as_ref()
            .expect("encoder is initialized during transducer load");
        while buf[p] != 0 {
            if encoder.find_key(&buf, &mut p).is_none() {
                return false;
            }
        }
        true
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.include-symbol-in-alphabet-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.include-symbol-in-alphabet-fn]
    pub fn include_symbol_in_alphabet(&mut self, sym: &str) {
        if self.get_alphabet().symbol_from_string(sym).is_some() {
            return;
        }
        let key = u32::try_from(self.get_alphabet().get_symbol_table().len())
            .expect("value out of u32 range") as SymbolNumber;
        self.alphabet
            .as_mut()
            .expect("alphabet is initialized during container load")
            .add_symbol_str(sym);
        self.encoder
            .as_mut()
            .expect("encoder is initialized during container load")
            .read_input_symbol(sym, key as i32);
    }

    // The convenience surface, for callers with one input and no interest in
    // reusing a state. An embedder that looks up a stream of words should hold
    // its own ['LookupState'] instead, which keeps the tape capacity and any
    // out-of-alphabet symbols it admitted.
    pub fn lookup_fd_strvec(
        &self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstOneLevelPaths {
        LookupState::new(self).lookup_fd_strvec(s, limit, time_cutoff)
    }

    pub fn lookup_fd_cstr(&self, s: &str, limit: isize, time_cutoff: f64) -> HfstOneLevelPaths {
        LookupState::new(self).lookup_fd(s, limit, time_cutoff)
    }

    pub fn lookup_fd_pairs_str(
        &self,
        s: &str,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstTwoLevelPaths {
        LookupState::new(self).lookup_fd_pairs(s, limit, time_cutoff)
    }

    pub fn is_lookup_infinitely_ambiguous_str(&self, s: &str) -> bool {
        LookupState::new(self).is_lookup_infinitely_ambiguous(s)
    }

    pub fn is_lookup_infinitely_ambiguous_strvec(&self, s: &StringVector) -> bool {
        LookupState::new(self).is_lookup_infinitely_ambiguous_strvec(s)
    }
}
