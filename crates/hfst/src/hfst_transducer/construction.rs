//! Facade constructors: wrapping, copying, basic-transducer conversion, and
//! the define_transducer_* shapes.

use super::*;

impl<B: Backend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Constructors -----
    // -------------------------------------------------------------------------

    /// Wrap an already-built backend in fresh facade metadata. Crate-visible
    /// for the stream readers and the OL conversion smugglers, which build the
    /// backend first.
    pub(crate) fn wrap(fst: B) -> Self {
        HfstTransducer {
            name: String::new(),
            props: BTreeMap::new(),
            anonymous: false,
            is_trie: true,
            fst,
        }
    }

    /// \brief Create an empty transducer.
    ///
    /// Covers both C++ constructors 'HfstTransducer()' (the UNSPECIFIED
    /// placeholder — a facade without a backend is no longer representable)
    /// and 'HfstTransducer(ImplementationType type)' (empty transducer of a
    /// type; the type is the parameter 'B' now).
    pub fn new() -> Self {
        Self::wrap(B::empty())
    }

    /// \brief Create a deep copy of transducer 'another'.
    ///
    /// 'HfstTransducer(const HfstTransducer &another)'.
    pub fn new_copy(another: &HfstTransducer<B>) -> crate::error::Result<Self> {
        let mut props = BTreeMap::new();
        for (k, v) in &another.props {
            if k.as_str() != "type" {
                props.insert(k.clone(), v.clone());
            }
        }
        // NOTE: like C++, 'name' stays "" even though 'props' may carry a copied
        // "name" entry.
        Ok(HfstTransducer {
            name: String::new(),
            props,
            anonymous: another.anonymous,
            is_trie: another.is_trie,
            fst: another.fst.copy()?,
        })
    }

    /// \brief Create an HFST transducer equivalent to HFST basic transducer
    /// 'net'.
    ///
    /// 'HfstTransducer(const hfst::implementations::HfstBasicTransducer &net, type)'.
    pub fn new_from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(HfstTransducer {
            name: net.name.clone(), // C++: name = net.name; (after the switch)
            props: BTreeMap::new(),
            anonymous: false,
            is_trie: false,
            fst: B::from_basic(net)?,
        })
    }

    /// [`Self::new_from_basic`] for a caller that is finished with `net`, so a
    /// backend can release the graph as it converts it.
    pub fn new_from_basic_owned(net: HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(HfstTransducer {
            name: net.name.clone(),
            props: BTreeMap::new(),
            anonymous: false,
            is_trie: false,
            fst: B::from_basic_owned(net)?,
        })
    }

    // -------------------------------------------------------------------------
    // ----- Assignment -----
    // -------------------------------------------------------------------------

    /// \brief Assign this transducer a new value equivalent to 'another'.
    ///
    /// 'HfstTransducer &operator=(const HfstTransducer &another)'. The C++
    /// type-mismatch check is compile-time now (both sides are the same 'B').
    pub fn operator_assign(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // Check for self-assignment.
        if std::ptr::eq(
            another as *const HfstTransducer<B>,
            self as *const HfstTransducer<B>,
        ) {
            return Ok(self);
        }

        // set some features
        self.anonymous = another.anonymous;
        self.is_trie = another.is_trie;
        let nm = another.get_name();
        self.set_name(&nm);

        // Set new transducer (the old backend is freed by the assignment).
        self.fst = another.fst.copy()?;
        Ok(self)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.universal-pair-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.universal-pair-fn]
    pub fn universal_pair() -> HfstTransducer<B> {
        let mut bt = HfstBasicTransducer::new();
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            Symbol::new_static("@_EPSILON_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_EPSILON_SYMBOL_@"),
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        bt.set_final_weight(1, &0.0);

        HfstTransducer::new_from_basic(&bt)
            .expect("converting a basic transducer to an available backend type cannot fail")
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.identity-pair-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.identity-pair-fn]
    pub fn identity_pair() -> HfstTransducer<B> {
        let mut bt = HfstBasicTransducer::new();
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        bt.set_final_weight(1, &0.0);

        HfstTransducer::new_from_basic(&bt)
            .expect("converting a basic transducer to an available backend type cannot fail")
    }
}

impl<B: Backend> Default for HfstTransducer<B> {
    fn default() -> Self {
        Self::new()
    }
}

// ===== integration shims: Clone (C++ copy ctor) =====
impl<B: Backend> Clone for HfstTransducer<B> {
    fn clone(&self) -> Self {
        HfstTransducer::new_copy(self).expect("cloning a valid transducer cannot fail")
    }
}

impl<B: AlgebraBackend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Construction constructors (define_transducer_* arms) -----
    // -------------------------------------------------------------------------

    /// 'HfstTransducer(const std::string &utf8_str, const HfstTokenizer&, type)'.
    pub fn new_tokenized(
        utf8_str: &str,
        multichar_symbol_tokenizer: &HfstTokenizer,
    ) -> crate::error::Result<Self> {
        if utf8_str.is_empty() {
            crate::bail!(
                EmptyString,
                "HfstTransducer(const std::string&, const HfstTokenizer&, ImplementationType)"
            );
        }
        let spv = multichar_symbol_tokenizer.tokenize(utf8_str, false);
        Ok(Self::wrap(B::define_transducer_spv(&spv)))
    }

    /// 'HfstTransducer(const std::string &upper, const std::string &lower,
    ///  const HfstTokenizer&, type)'.
    pub fn new_tokenized_pair(
        upper_utf8_str: &str,
        lower_utf8_str: &str,
        multichar_symbol_tokenizer: &HfstTokenizer,
    ) -> crate::error::Result<Self> {
        if upper_utf8_str.is_empty() || lower_utf8_str.is_empty() {
            // NOTE: the C++ message is missing its closing paren; preserved.
            crate::bail!(
                EmptyString,
                "HfstTransducer(const std::string&, const std::string&, const HfstTokenizer&, ImplementationType"
            );
        }
        let spv = multichar_symbol_tokenizer.tokenize_pair(upper_utf8_str, lower_utf8_str, false);
        Ok(Self::wrap(B::define_transducer_spv(&spv)))
    }

    /// 'HfstTransducer(const StringPairSet &sps, type, bool cyclic=false)'.
    pub fn new_string_pair_set(sps: &StringPairSet, cyclic: bool) -> crate::error::Result<Self> {
        for sp in sps {
            if sp.0.is_empty() || sp.1.is_empty() {
                crate::bail!(
                    EmptyString,
                    "HfstTransducer(const StringPairSet&, ImplementationType, bool)"
                );
            }
        }
        let mut t = Self::wrap(B::define_transducer_sps(sps, cyclic));
        t.is_trie = false;
        Ok(t)
    }

    /// 'HfstTransducer(const StringPairVector &spv, type)'.
    pub fn new_string_pair_vector(spv: &StringPairVector) -> crate::error::Result<Self> {
        for it in spv {
            if it.0.is_empty() || it.1.is_empty() {
                crate::bail!(
                    EmptyString,
                    "HfstTransducer(const StringPairVector&, ImplementationType)"
                );
            }
        }
        let mut t = Self::wrap(B::define_transducer_spv(spv));
        t.is_trie = false;
        Ok(t)
    }

    /// 'HfstTransducer(const StringVector &sv, type)'.
    ///
    /// C++ builds 'spv' then does '*this = HfstTransducer(spv, type)' on an
    /// uninitialized placeholder; the placeholder is a real empty transducer
    /// now, and 'operator_assign' reproduces the observable result
    /// ('props["name"] == ""', the copied backend).
    pub fn new_string_vector(sv: &StringVector) -> crate::error::Result<Self> {
        let mut this = Self::new();
        this.is_trie = false;
        let mut spv = StringPairVector::new();
        for it in sv {
            spv.push((it.clone(), it.clone()));
        }
        // *this = HfstTransducer(spv, type);
        let tmp = Self::new_string_pair_vector(&spv)?;
        this.operator_assign(&tmp)?;
        Ok(this)
    }

    /// 'HfstTransducer(const std::vector<StringPairSet> &spsv, type)'.
    pub fn new_string_pair_set_vector(spsv: &[StringPairSet]) -> crate::error::Result<Self> {
        for it in spsv {
            for pair in it {
                if pair.0.is_empty() || pair.1.is_empty() {
                    crate::bail!(
                        EmptyString,
                        "HfstTransducer(const std::vector<StringPairSet>&, ImplementationType)"
                    );
                }
            }
        }
        let mut t = Self::wrap(B::define_transducer_spsv(spsv));
        t.is_trie = false;
        Ok(t)
    }

    /// \brief Create '[symbol:symbol]'.
    ///
    /// 'HfstTransducer(const std::string &symbol, type)'.
    pub fn new_symbol(symbol: &str) -> crate::error::Result<Self> {
        if symbol.is_empty() {
            crate::bail!(
                EmptyString,
                "HfstTransducer(const std::string&, ImplementationType)"
            );
        }
        let mut t = Self::wrap(B::define_transducer_symbol(symbol));
        t.is_trie = false;
        Ok(t)
    }

    /// \brief Create '[isymbol:osymbol]'.
    ///
    /// 'HfstTransducer(const std::string &isymbol, const std::string &osymbol, type)'.
    pub fn new_symbol_pair(isymbol: &str, osymbol: &str) -> crate::error::Result<Self> {
        if isymbol.is_empty() || osymbol.is_empty() {
            crate::bail!(
                EmptyString,
                "HfstTransducer(const std::string&, const std::string&,  ImplementationType)"
            );
        }
        let mut t = Self::wrap(B::define_transducer_symbol_pair(isymbol, osymbol));
        t.is_trie = false;
        Ok(t)
    }
}
