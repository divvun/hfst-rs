//! The load-fixed half of the pmatch runtime.
//!
//! The C++ `hfst_ol::PmatchContainer` was one object: the alphabet, the encoder,
//! the toplevel net and its RTNs sat on the same struct as the tapes, the
//! local-variable stacks, the capture bookkeeping and the line counter. Matching
//! therefore took the whole container exclusively, and a container big enough to
//! be worth sharing — a giellacg tokeniser runs to hundreds of megabytes of
//! tables — could only be used by a second thread by loading it again.
//!
//! [`PmatchCore`] is everything that stops changing once loading finishes. It is
//! handed out behind an [`Arc`](std::sync::Arc) and read by any number of
//! [`PmatchContainer`](crate::pmatch::PmatchContainer) run states at once; every
//! byte a match writes lives on the run state instead.
//!
//! Loading itself still mutates: an `@L.` symbol list is tokenized against the
//! alphabet it is being added to, and an initial-symbol list can admit symbols
//! the archive never declared. Those run against `&mut PmatchCore` before the
//! `Arc` exists, which is why [`SymbolAdmitter`] has a growing implementation
//! here and an overlay implementation on the run state.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]

use std::collections::BTreeMap;

use tracing::warn;

use crate::hfst_data_types::Symbol;
use crate::hfst_flag_diacritics::FdState;
use crate::pmatch::{
    PmatchAlphabet, PmatchContainer, PmatchTransducer, SpecialSymbol, nByte_grapheme,
    nByte_grapheme_bytes,
};
use crate::transducer::{Encoder, NO_SYMBOL_NUMBER, SymbolNumber, SymbolNumberVector};

/// The matching behaviour an archive declares in its header.
///
/// Written while the archive is read and then frozen into the core as the
/// defaults every run starts from; a run state keeps its own copy so the
/// `set_*` knobs on one run are invisible to another.
// [spec:hfst:def:pmatch.hfst-ol.pmatch-container]
#[derive(Clone, Copy)]
pub struct PmatchProperties {
    pub count_patterns: bool,
    pub delete_patterns: bool,
    pub extract_patterns: bool,
    pub locate_mode: bool,
    pub mark_patterns: bool,
    pub max_context_length: usize,
    pub max_recursion: usize,
    pub need_separators: bool,
    pub xerox_composition: bool,
    pub uncomposable: bool,
}

impl Default for PmatchProperties {
    fn default() -> Self {
        Self::new()
    }
}

impl PmatchProperties {
    // void set_properties(void)
    pub fn new() -> PmatchProperties {
        PmatchProperties {
            count_patterns: false,
            delete_patterns: false,
            extract_patterns: false,
            locate_mode: false,
            mark_patterns: true,
            max_context_length: 254,
            max_recursion: 5000,
            need_separators: true,
            xerox_composition: true,
            uncomposable: false,
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-properties-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-properties-fn]
    // void set_properties(std::map<std::string, std::string> &)
    pub fn set_from_map(&mut self, properties: &BTreeMap<String, String>) {
        for (first, second) in properties.iter() {
            if first == "count-patterns" {
                if second == "on" {
                    self.count_patterns = true;
                } else if second == "off" {
                    self.count_patterns = false;
                }
            } else if first == "delete-patterns" {
                if second == "on" {
                    self.delete_patterns = true;
                } else if second == "off" {
                    self.delete_patterns = false;
                }
            } else if first == "extract-patterns" {
                if second == "on" {
                    self.extract_patterns = true;
                } else if second == "off" {
                    self.extract_patterns = false;
                }
            } else if first == "locate-patterns" {
                if second == "on" {
                    self.locate_mode = true;
                } else if second == "off" {
                    self.locate_mode = false;
                }
            } else if first == "mark-patterns" {
                if second == "on" {
                    self.mark_patterns = true;
                } else if second == "off" {
                    self.mark_patterns = false;
                }
            } else if first == "max-context-length" {
                // std::stringstream converter(it->second); converter >> max_context_length;
                match second.trim().parse::<usize>() {
                    Ok(v) => {
                        self.max_context_length = v;
                        if self.max_context_length == 0 && second != "0" {
                            self.max_context_length = 254;
                        }
                    }
                    Err(_) => {
                        // failed extraction leaves value 0 (as a freshly default-
                        // initialized stringstream target would)
                        self.max_context_length = 0;
                        if second != "0" {
                            self.max_context_length = 254;
                        }
                    }
                }
            } else if first == "max-recursion" {
                match second.trim().parse::<usize>() {
                    Ok(v) => {
                        self.max_recursion = v;
                        if self.max_recursion == 0 && second != "0" {
                            self.max_recursion = 5000;
                        }
                    }
                    Err(_) => {
                        self.max_recursion = 0;
                        if second != "0" {
                            self.max_recursion = 5000;
                        }
                    }
                }
            } else if first == "need-separators" {
                if second == "on" {
                    self.need_separators = true;
                } else if second == "off" {
                    self.need_separators = false;
                }
            } else if first == "xerox-composition" {
                if second == "off" {
                    self.xerox_composition = false;
                } else if second == "on" {
                    self.xerox_composition = true;
                }
            }
        }
    }
}

/// Everything the pmatch runtime reads and never writes once an archive is
/// loaded: the alphabet with its RTNs, the encoder that tokenizes input, the
/// toplevel net, the uncomposer nets and the header-declared defaults.
///
/// Shared behind an `Arc` by every run state over this archive. Nothing here is
/// reachable through a `&mut` once the `Arc` exists, so concurrent matches
/// need no lock around the tables.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
pub struct PmatchCore {
    pub(crate) alphabet: PmatchAlphabet,
    pub(crate) encoder: Option<Encoder>,
    pub(crate) orig_symbol_count: SymbolNumber,
    /// How many symbols the alphabet holds once loading is done, which is where
    /// a run state's out-of-alphabet overlay starts numbering.
    pub(crate) symbol_count: SymbolNumber,
    pub(crate) toplevel: Option<Box<PmatchTransducer>>,
    // C++ raw 'hfst_ol::Transducer *'; the two uncomposer nets read by
    // 'uncompose' via 'lookup_fd'. Owned box, optional.
    pub(crate) uncompose_left: Option<Box<crate::transducer::Transducer>>,
    pub(crate) uncompose_right: Option<Box<crate::transducer::Transducer>>,
    pub(crate) possible_first_symbols: Vec<bool>,
    pub(crate) props: PmatchProperties,
    /// The neutral flag-diacritic state this alphabet defines, built once at
    /// load. `FdState::new` deep-copies the flag table, so every run state and
    /// every RTN frame clones this instead — a refcount bump on the table.
    pub(crate) flag_state_proto: FdState<SymbolNumber>,
}

impl Default for PmatchCore {
    fn default() -> Self {
        Self::new()
    }
}

/// How the input tokenizer turns a spelling it cannot find into a symbol
/// number.
///
/// The two implementations differ only in where the new symbol lands: loading
/// puts it in the alphabet itself, a run puts it in that run's overlay.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
pub(crate) trait SymbolAdmitter {
    fn find_key(&self, buf: &[u8], p: &mut usize) -> Option<SymbolNumber>;
    fn admit(&mut self, symbol: Symbol) -> SymbolNumber;
}

// [spec:hfst:def:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
// [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
pub(crate) fn tokenize_input(
    sink: &mut impl SymbolAdmitter,
    boundary: SymbolNumber,
    single_codepoint_tokenization: bool,
    input_s: &str,
    out: &mut SymbolNumberVector,
) {
    out.clear();
    // The C++ walks a 0-terminated char buffer with a pointer it advances.
    let mut buf: Vec<u8> = input_s.as_bytes().to_vec();
    buf.push(0);
    let mut p: usize = 0;
    // 'k' lives outside the loop to mirror the C++, where a stale value can
    // carry over when single-codepoint tokenization finds no bytes to take.
    let mut k: Option<SymbolNumber> = None;
    if boundary != NO_SYMBOL_NUMBER {
        out.push(boundary);
    }
    while buf[p] != 0 {
        let original_input_loc = p;
        if single_codepoint_tokenization {
            let bytes_to_tokenize = nByte_grapheme_bytes(&buf[p..buf.len() - 1]);
            if bytes_to_tokenize > 0 {
                // memcpy the first bytes_to_tokenize bytes, NUL terminate,
                // then find_key on that scratch buffer.
                let mut scratch: Vec<u8> = buf[p..p + bytes_to_tokenize as usize].to_vec();
                scratch.push(0);
                let mut sp: usize = 0;
                k = sink.find_key(&scratch, &mut sp);
                if k.is_some() {
                    p += bytes_to_tokenize as usize;
                }
            }
        } else {
            k = sink.find_key(&buf, &mut p);
        }
        let key = match k {
            Some(key) => key,
            None => {
                // Regular tokenization failed
                p = original_input_loc;
                let mut bytes_to_tokenize = nByte_grapheme_bytes(&buf[p..buf.len() - 1]);
                if bytes_to_tokenize == 0 {
                    // if utf-8 tokenization fails too, just grab a byte
                    bytes_to_tokenize = 1;
                }
                let new_symbol_bytes = buf[p..p + bytes_to_tokenize as usize].to_vec();
                let new_symbol =
                    Symbol::from(String::from_utf8_lossy(&new_symbol_bytes).into_owned());
                p += bytes_to_tokenize as usize;
                let key = sink.admit(new_symbol);
                k = Some(key);
                key
            }
        };
        out.push(key);
    }
    if boundary != NO_SYMBOL_NUMBER {
        out.push(boundary);
    }
}

/// Tokenizing during load grows the alphabet, the encoder and the symbol count
/// in place — the archive is still being read, so there is nothing to share
/// yet and no overlay to keep the growth out of.
impl SymbolAdmitter for PmatchCore {
    fn find_key(&self, buf: &[u8], p: &mut usize) -> Option<SymbolNumber> {
        self.encoder
            .as_ref()
            .expect("encoder is initialized during container load")
            .find_key(buf, p)
    }

    fn admit(&mut self, symbol: Symbol) -> SymbolNumber {
        self.alphabet.add_symbol(&symbol);
        self.encoder
            .as_mut()
            .expect("encoder is initialized during container load")
            .read_input_symbol(&symbol, self.symbol_count as i32);
        let key = self.symbol_count;
        self.symbol_count += 1;
        key
    }
}

impl PmatchCore {
    // PmatchContainer(void)
    pub fn new() -> PmatchCore {
        PmatchCore {
            alphabet: PmatchAlphabet::new(),
            encoder: None,
            orig_symbol_count: 0,
            symbol_count: 0,
            toplevel: None,
            uncompose_left: None,
            uncompose_right: None,
            possible_first_symbols: Vec::new(),
            props: PmatchProperties::new(),
            flag_state_proto: FdState::new_default(),
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // explicit PmatchContainer(std::istream &) — reads a binary pmatch archive:
    // the TOP transducer followed by any UNCOMPOSE L/R nets and RTN sub-nets.
    pub fn from_stream(
        is: &mut crate::transducer::IStream<'_>,
    ) -> crate::error::Result<PmatchCore> {
        use crate::transducer::{TransducerAlphabet, TransducerHeader};
        let mut c = PmatchCore::new();
        let mut properties = PmatchContainer::parse_hfst3_header(is)?;
        let transducer_name: String;
        if !properties.contains_key("name") {
            warn!("TOP not defined in archive, using first as TOP");
            transducer_name = "TOP".to_string();
        } else {
            transducer_name = properties["name"].clone();
            if transducer_name != "TOP" {
                warn!("TOP not defined in archive, using first as TOP");
            }
        }
        let _ = transducer_name;
        if !properties.contains_key("type") {
            warn!("type information missing from archive");
        } else if properties["type"] != "HFST_OLW" {
            warn!("archive type isn't weighted optimized-lookup according to header");
        }
        c.props.set_from_map(&properties);
        let header = TransducerHeader::new_istream(is)?;
        c.alphabet = PmatchAlphabet::new_from_stream(is, header.symbol_count(), &mut c)?;
        // Every compiled pmatch TOP is wrapped in @PMATCH_ENTRY@/@PMATCH_EXIT@
        // (`add_pmatch_delimiters`); those markers are what delimits a match, so
        // an archive lacking them cannot report one however far the walk gets.
        // Their absence means the stream is some other optimized-lookup
        // transducer — a plain .hfstol analyser handed to `hfst tokenize`, say —
        // and naming that beats matching nothing.
        if c.alphabet.get_special(SpecialSymbol::entry) == NO_SYMBOL_NUMBER
            || c.alphabet.get_special(SpecialSymbol::exit) == NO_SYMBOL_NUMBER
        {
            crate::bail!(
                Hfst,
                "not a pmatch archive: the transducer's alphabet carries no @PMATCH_ENTRY@/@PMATCH_EXIT@ markers (an optimized-lookup transducer that was not compiled by hfst-pmatch2fst?)"
            );
        }
        c.orig_symbol_count = c.alphabet.get_orig_symbol_count();
        c.symbol_count = c.alphabet.get_orig_symbol_count();
        c.flag_state_proto = FdState::new(c.alphabet.get_fd_table());
        c.encoder = Some(Encoder::new(
            c.alphabet.get_symbol_table(),
            c.orig_symbol_count,
        ));
        if properties.contains_key("initial-symbols") {
            let initial = properties["initial-symbols"].clone();
            c.collect_first_symbols(&initial);
        }
        let top = PmatchTransducer::new_from_stream(
            is,
            header.index_table_size(),
            header.target_table_size(),
            &c.alphabet,
            "TOP".to_string(),
        )?;
        c.toplevel = Some(Box::new(top));
        // C++ loops 'while (inputstream.good())' reading further archive members,
        // breaking when parse_hfst3_header throws TransducerHeaderException. A
        // well-formed archive ends in a clean EOF right after the last member, so
        // peek for end-of-stream (now possible via get/putback) instead of
        // catching the throw.
        loop {
            if !is.good() {
                break;
            }
            let probe = is.get();
            if probe < 0 {
                break;
            }
            is.putback(probe as u8);
            properties = PmatchContainer::parse_hfst3_header(is)?;
            let transducer_name = properties.get("name").cloned().unwrap_or_default();
            if transducer_name.starts_with("UNCOMPOSE LEFT") {
                c.uncompose_left = Some(Box::new(crate::transducer::Transducer::new_istream(is)?));
                c.props.uncomposable = true;
            } else if transducer_name.starts_with("UNCOMPOSE RIGHT") {
                c.uncompose_right = Some(Box::new(crate::transducer::Transducer::new_istream(is)?));
                c.props.uncomposable = true;
            } else {
                let rtn_header = TransducerHeader::new_istream(is)?;
                let _dummy = TransducerAlphabet::new_istream(is, rtn_header.symbol_count(), true)?;
                let rtn = PmatchTransducer::new_from_stream(
                    is,
                    rtn_header.index_table_size(),
                    rtn_header.target_table_size(),
                    &c.alphabet,
                    transducer_name.clone(),
                )?;
                if !c.alphabet.has_rtn(&transducer_name) {
                    c.alphabet.add_rtn(Box::new(rtn), &transducer_name);
                }
                // else: C++ 'delete rtn' — Rust drops it here.
            }
        }
        Ok(c)
    }

    // PmatchContainer(Transducer *t)
    pub fn from_transducer(
        toplevel: crate::transducer::Transducer,
    ) -> crate::error::Result<PmatchCore> {
        let mut c = PmatchCore::new();
        // TransducerHeader header = t->get_header();
        c.alphabet = PmatchAlphabet::new_from_alphabet(toplevel.get_alphabet(), &mut c);
        c.orig_symbol_count = c.alphabet.get_orig_symbol_count();
        c.symbol_count = c.alphabet.get_orig_symbol_count();
        c.flag_state_proto = FdState::new(c.alphabet.get_fd_table());
        c.encoder = Some(Encoder::new(
            c.alphabet.get_symbol_table(),
            c.orig_symbol_count,
        ));
        // [spec:hfst:req:table-residency.single-copy-load]
        let (indices, transitions) = toplevel.into_weighted_tables()?.into_tables();
        let top = PmatchTransducer::new_from_vectors(
            transitions.into_vector(),
            indices.into_vector(),
            &c.alphabet,
            "TOP".to_string(),
        );
        c.toplevel = Some(Box::new(top));
        Ok(c)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // explicit PmatchContainer(std::vector<hfst::HfstTransducer>)
    pub fn from_hfst_transducers(
        transducers: Vec<crate::hfst_transducer::HfstTransducer<crate::transducer::Transducer>>,
    ) -> crate::error::Result<PmatchCore> {
        if transducers.is_empty() {
            return Ok(PmatchCore::new());
        }
        if transducers.len() == 1 {
            // C++: convert transducers[0] to HFST_OLW (unless already), then
            // hfst_transducer_to_hfst_ol(top) to get the optimized-lookup backend,
            // and build the container from it (exactly from_transducer). The
            // pmatch runtime pins the weighted optimized-lookup instantiation
            // ([dec:hfst:monomorphic-backends]), so the runtime convert is a
            // static fact of the parameter type now.
            let top = &transducers[0];
            let backend = crate::transducer::Transducer::copy(&top.fst)?;
            let mut c = PmatchCore::from_transducer(backend)?;
            // C++ sets these from transducers[0]'s properties before building; the
            // build does not depend on them, so applying them afterwards is
            // equivalent.
            c.props.set_from_map(transducers[0].get_properties());
            Ok(c)
        } else {
            // This is the difficult case where we have to make sure multiple
            // optimized-lookup transducers are harmonized with each other.
            use crate::convert_transducer_format::ConversionFunctions;

            let mut c = PmatchCore::new();
            c.props.set_from_map(transducers[0].get_properties());

            // A dummy transducer with an alphabet with all the symbols
            // (TROPICAL_OPENFST_TYPE in C++; the tropical backend type here).
            let mut harmonizer: crate::hfst_transducer::HfstTransducer<hfst_openfst::StdVectorFst> =
                crate::hfst_transducer::HfstTransducer::new();
            // First we need to collect a unified alphabet from all the
            // transducers.
            let mut symbols_seen: std::collections::BTreeSet<Symbol> =
                std::collections::BTreeSet::new();
            // The TOP member: the last transducer named "TOP" (NULL == none).
            let mut top_index: Option<usize> = None;
            // We collect all the symbols and also locate the TOP member.
            for (i, transducer) in transducers.iter().enumerate() {
                let string_set = transducer.get_alphabet()?;
                for sym in string_set.iter() {
                    if !symbols_seen.contains(sym) {
                        let ht = crate::hfst_transducer::HfstTransducer::new_symbol(sym)?;
                        harmonizer.disjunct(&ht, true)?;
                        symbols_seen.insert(sym.clone());
                    }
                }
                if transducer.get_name() == "TOP" {
                    top_index = Some(i);
                }
            }
            let top_index = match top_index {
                Some(i) => i,
                None => {
                    warn!("TOP not defined in archive, using first as TOP");
                    0
                }
            };
            // Then we convert the harmonizer... (typed now: basic -> weighted
            // OL tables, the former convert(HFST_OLW_TYPE)).
            let harmonizer_net = harmonizer.get_basic_transducer()?;
            let harmonizer_ol_owned = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                &harmonizer_net,
                true,
                "",
                None,
            )?;
            let harmonizer_ol = &harmonizer_ol_owned;

            // We take care of TOP first. Already OLW by type (the C++ convert
            // is static); go through an intermediate basic transducer, then
            // harmonize into OL.
            let top = transducers[top_index].clone();
            let intermediate_tmp =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&top)?;
            let harmonized_tmp = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                &intermediate_tmp,
                true,                // weighted
                "",                  // no special options
                Some(harmonizer_ol), // harmonize with this
            )?;
            // this will be the alphabet of the entire container
            c.alphabet = PmatchAlphabet::new_from_alphabet(harmonized_tmp.get_alphabet(), &mut c);
            c.orig_symbol_count = c.alphabet.get_orig_symbol_count();
            c.symbol_count = c.alphabet.get_orig_symbol_count();
            c.flag_state_proto = FdState::new(c.alphabet.get_fd_table());
            c.encoder = Some(Encoder::new(
                c.alphabet.get_symbol_table(),
                c.orig_symbol_count,
            ));
            // [spec:hfst:req:table-residency.single-copy-load]
            let (indices, transitions) = harmonized_tmp.into_weighted_tables()?.into_tables();
            let top_pt = PmatchTransducer::new_from_vectors(
                transitions.into_vector(),
                indices.into_vector(),
                &c.alphabet,
                "TOP".to_string(),
            );
            c.toplevel = Some(Box::new(top_pt));
            // Then we do the same for the other transducers except without
            // alphabets or encoders because those should be identical. Members
            // named "TOP" left a NULL slot in the C++ 'temporaries' vector and
            // are skipped here.
            for transducer in &transducers {
                if transducer.get_name() == "TOP" {
                    // there's a NULL where TOP should be
                    continue;
                }
                let temp = transducer.clone();
                let intermediate_tmp =
                    ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&temp)?;
                let harmonized_tmp = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                    &intermediate_tmp,
                    true,
                    "",
                    Some(harmonizer_ol),
                )?;
                // [spec:hfst:req:table-residency.single-copy-load]
                let (indices, transitions) = harmonized_tmp.into_weighted_tables()?.into_tables();
                let name = transducer.get_name();
                let rtn = PmatchTransducer::new_from_vectors(
                    transitions.into_vector(),
                    indices.into_vector(),
                    &c.alphabet,
                    name.clone(),
                );
                c.alphabet.add_rtn(Box::new(rtn), &name);
            }
            Ok(c)
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.add-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.add-rtn-fn]
    pub fn add_rtn(
        &mut self,
        rtn: &crate::transducer::Transducer,
        name: &str,
    ) -> crate::error::Result<()> {
        // The argument is borrowed and outlives the call, so the copy the
        // copy_*_table pair makes is the one copy this construction needs.
        // [spec:hfst:req:table-residency.single-copy-load]
        let transitions = rtn.copy_transitionw_table()?;
        let indices = rtn.copy_windex_table()?;
        let pmatch_rtn = Box::new(PmatchTransducer::new_from_vectors(
            transitions.into_vector(),
            indices.into_vector(),
            &self.alphabet,
            name.to_string(),
        ));
        if !self.alphabet.has_rtn(name) {
            self.alphabet.add_rtn(pmatch_rtn, name);
        } else {
            // C++ does 'delete rtn;' here (note: deletes the *argument*, not the
            // freshly-built pmatch_rtn — a faithful bug). We own neither; the
            // argument is borrowed and pmatch_rtn is simply dropped.
            drop(pmatch_rtn);
        }
        Ok(())
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.collect-first-symbols-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.collect-first-symbols-fn]
    pub fn collect_first_symbols(&mut self, symbol_list: &str) {
        let first_symbols = self.symbol_vector_from_symbols(symbol_list);
        for &it in first_symbols.iter() {
            while it as usize >= self.possible_first_symbols.len() {
                self.possible_first_symbols.push(false);
            }
            self.possible_first_symbols[it as usize] = true;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.symbol-vector-from-symbols-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.symbol-vector-from-symbols-fn]
    pub fn symbol_vector_from_symbols(&mut self, symbols: &str) -> SymbolNumberVector {
        let boundary = self.alphabet.get_special(SpecialSymbol::boundary);
        let mut input = SymbolNumberVector::new();
        tokenize_input(self, boundary, false, symbols, &mut input);
        if boundary != NO_SYMBOL_NUMBER {
            // SymbolNumberVector(input.begin() + 1, input.end() - 1)
            return input[1..input.len() - 1].to_vec();
        }
        input
    }

    /// How many symbols the loaded alphabet names. Matching never moves this —
    /// a symbol a run has to invent goes in that run's overlay instead.
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn symbol_count(&self) -> SymbolNumber {
        self.symbol_count
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.not-possible-first-symbol-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.not-possible-first-symbol-fn]
    pub fn not_possible_first_symbol(&self, sym: SymbolNumber) -> bool {
        if self.possible_first_symbols.is_empty() {
            return false;
        }
        (sym as usize) >= self.possible_first_symbols.len()
            || !self.possible_first_symbols[sym as usize]
    }

    // Is this symbol a plain text symbol whose surface spans more than one
    // grapheme cluster (e.g. a multichar symbol like "dz")? Composed characters
    // — precomposed OR decomposed, each a single grapheme cluster — are not.
    // Epsilon / flag / `@...@` special symbols are not input text at all.
    fn is_multichar_text_symbol(&self, sym: SymbolNumber) -> bool {
        let Some(s) = self.alphabet.get_symbol_table().get(sym as usize) else {
            return false;
        };
        if s.is_empty() || crate::hfst_symbol_defs::is_epsilon(s) {
            return false;
        }
        // @...@ specials (flags, insertions, ENDTAG, IDENTITY/UNKNOWN, ...)
        // are never plain input text.
        if s.starts_with('@') && s.ends_with('@') {
            return false;
        }
        if self.alphabet.is_flag_diacritic(sym) {
            return false;
        }
        let first = nByte_grapheme(s);
        first > 0 && (first as usize) < s.len()
    }

    // [DIVERGENCE hfst/hfst#367] Can this network consume a multichar text
    // symbol from the input tape? Such symbols only match under longest-match
    // ('multichar') tokenization; single-codepoint tokenization segments one
    // grapheme at a time and never sees them, so `tokenise` would drop words
    // that `lookup` accepts. The tokeniser uses this to pick the default mode
    // when the user did not force `--tokenize-multichar`.
    //
    // The question is deliberately asked of the *transition* input sides, not of
    // the symbol table. A pmatch archive's symbol table is shared between both
    // tapes, so an Ins-ed analyser contributes its whole analysis vocabulary to
    // it — and a giellacg tokeniser's tag inventory contains space-prefixed
    // multichar symbols like " N" / " A" / " V". Judging by the symbol table
    // alone, every such archive looks multichar, longest-match tokenization then
    // glues the space onto the next word's initial, and the surrounding words
    // stop matching entirely. Only symbols some arc can actually read — directly
    // or as a member of a symbol list (`Lst`) an arc reads — are evidence that
    // longest-match segmentation is needed. Exclusionary lists (`Exc`) are not:
    // they are character-class constructs that match by exclusion.
    pub fn has_multichar_input_symbols(&self) -> bool {
        let table_len = self.alphabet.get_symbol_table().len();
        let multichar: Vec<bool> = (0..table_len)
            .map(|i| self.is_multichar_text_symbol(i as SymbolNumber))
            .collect();
        if !multichar.iter().any(|m| *m) {
            return false;
        }
        let readable_as_multichar = |sym: SymbolNumber| -> bool {
            let i = sym as usize;
            if i >= table_len {
                return false;
            }
            if multichar[i] {
                return true;
            }
            let list = self
                .alphabet
                .list2symbols
                .get(i)
                .copied()
                .unwrap_or(NO_SYMBOL_NUMBER);
            if list == NO_SYMBOL_NUMBER {
                return false;
            }
            self.alphabet
                .symbol_list_members
                .get(list as usize)
                .is_some_and(|members| {
                    members
                        .iter()
                        .any(|m| (*m as usize) < table_len && multichar[*m as usize])
                })
        };
        let nets = self
            .toplevel
            .as_deref()
            .into_iter()
            .chain(self.alphabet.rtns.iter().filter_map(|r| r.as_deref()));
        for net in nets {
            for transition in net.transition_table.iter() {
                if readable_as_multichar(transition.get_input_symbol()) {
                    return true;
                }
            }
        }
        false
    }
}
