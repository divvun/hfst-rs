//! Path extraction and the optimized-lookup surface.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: Backend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Path extraction -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-paths-fn]
    pub fn extract_paths_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
    ) -> crate::error::Result<()> {
        self.fst.extract_paths_cb(callback, cycles);
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-paths-fd-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-paths-fd-fn]
    pub fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) -> crate::error::Result<()> {
        self.fst.extract_paths_fd_cb(callback, cycles, filter_fd);
        Ok(())
    }

    pub fn extract_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        cycles: i32,
    ) -> crate::error::Result<()> {
        if self.is_cyclic()? && max_num < 1 && cycles < 0 {
            crate::bail!(TransducerIsCyclic, "HfstTransducer::extract_paths");
        }

        let mut cb = ExtractStringsCb_::new(results, max_num);
        self.extract_paths_cb(&mut cb, cycles)?;
        Ok(())
    }

    pub fn extract_paths_fd(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        cycles: i32,
        filter_fd: bool,
    ) -> crate::error::Result<()> {
        if self.is_cyclic()? && max_num < 1 && cycles < 0 {
            crate::bail!(TransducerIsCyclic, "HfstTransducer::extract_paths_fd");
        }

        let mut cb = ExtractStringsCb_::new(results, max_num);
        self.extract_paths_fd_cb(&mut cb, cycles, filter_fd)?;
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-shortest-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-shortest-paths-fn]
    pub fn extract_shortest_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
    ) -> crate::error::Result<()> {
        // The C++ converted a copy to TROPICAL_OPENFST_TYPE before n_best; the
        // conversion is typed now ([dec:hfst:monomorphic-backends]).
        let mut t: HfstTransducer<StdVectorFst> = HfstTransducer::wrap(
            <StdVectorFst as Backend>::from_basic(&self.fst.to_basic()?)?,
        );
        t.n_best(1)?;
        t.extract_paths(results, -1, -1)?;
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fd-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fd-fn]
    pub fn extract_random_paths_fd(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        filter_fd: bool,
    ) -> crate::error::Result<()> {
        // The C++ converted a copy to TROPICAL_OPENFST_TYPE (the only backend
        // with a fd-filtered random extraction); the conversion is typed now.
        let copy: StdVectorFst = <StdVectorFst as Backend>::from_basic(&self.fst.to_basic()?)?;
        TropicalWeightTransducer::extract_random_paths_fd(&copy, results, max_num, filter_fd);
        Ok(())
    }
}

impl<B: AlgebraBackend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Longest / random / n-best paths -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.longest-path-size-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.longest-path-size-fn]
    pub fn longest_path_size(&self, obey_flags: bool) -> crate::error::Result<i32> {
        if self.is_cyclic()? {
            crate::bail!(TransducerIsCyclic);
        }

        if !obey_flags {
            let net = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
            return Ok(net.longest_path_size());
        }

        let mut results = HfstTwoLevelPaths::new();
        let paths_found = self.extract_longest_paths(&mut results, true /* obey flags */)?;
        if !paths_found {
            return Ok(-1);
        }
        // else, there is at least one path
        Ok(results
            .iter()
            .next()
            .expect("paths_found is true, so results has at least one entry")
            .second
            .len() as i32)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-longest-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-longest-paths-fn]
    pub fn extract_longest_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
        obey_flags: bool, /*,show_flags: bool*/
    ) -> crate::error::Result<bool> {
        if self.is_cyclic()? {
            crate::bail!(TransducerIsCyclic);
        }

        let net = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self)?;
        let path_lengths = net.path_sizes();
        if path_lengths.is_empty() {
            return Ok(false);
        }

        let flags = net.get_flags();

        // go through each length of accepted paths in descending order
        for path_length in path_lengths.iter().copied() {
            // create a transducer [ any any ... any any ] where the number of
            // transitions that accept any symbol (including flags) is equal to
            // current length of accepted paths
            let match_length = match_any_n_times(path_length, &flags);

            let mut xre = crate::xre::XreCompiler::<B>::new();
            let mut length_tr: HfstTransducer<B> = xre
                .compile(match_length.as_str())
                .expect("match_any_n_times builds a well-formed xre");

            // filter out the paths of current length and extract them
            length_tr.compose(self, true)?;
            length_tr.optimize()?;
            if obey_flags {
                length_tr.extract_paths_fd(results, -1, -1, true)?;
            } else {
                length_tr.extract_paths(results, -1, -1)?;
            }

            // if paths were found
            if !results.is_empty() {
                return Ok(true);
            }
        } // lengths of accepted paths gone through

        // no paths found
        Ok(false)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fn]
    pub fn extract_random_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
    ) -> crate::error::Result<()> {
        // (The C++ round-tripped SFST and foma through TROPICAL_OPENFST_TYPE
        // to borrow its implementation. Each backend answers for itself here;
        // foma's unweighted reading is documented on its impl.)
        self.fst.extract_random_paths(results, max_num);
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// The lookup surface — only on the two optimized-lookup instantiations.
// -----------------------------------------------------------------------------

macro_rules! ol_lookup_facade {
    ($tables:ty) => {
        impl HfstTransducer<Transducer<$tables>> {
            /// A run state over this transducer's optimized-lookup machine —
            /// the reusable form of the lookup methods below, for a caller
            /// working through a stream of inputs.
            // [spec:hfst:req:lookup-run-state.caller-owned-scratch]
            pub fn lookup_state(&self) -> LookupState<'_, $tables> {
                LookupState::new(&self.fst)
            }

            /// Whether `s` tokenizes into symbols this transducer already has.
            /// Unlike the lookup methods, this cannot grow the alphabet — see
            /// [`Transducer::can_tokenize`] for why the distinction is
            /// observable.
            pub fn can_tokenize(&self, s: &str) -> bool {
                self.fst.can_tokenize(s)
            }

            // The lookup methods take '&self': the C++ exposed them as const
            // and mutated the transducer through a const-cast, and the port
            // long took '&mut self' because an out-of-alphabet input really did
            // grow the alphabet. It no longer does — the admission lives in the
            // run state — so shared access is the truth here.
            // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-pairs-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-pairs-fn]
            pub fn lookup_pairs(
                &self,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> HfstTwoLevelPaths {
                self.fst.lookup_fd_pairs_str(s, limit, time_cutoff)
            }

            // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-fd-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-fd-fn]
            pub fn lookup_fd_string_vector(
                &self,
                s: &StringVector,
                limit: isize,
                time_cutoff: f64,
            ) -> crate::error::Result<HfstOneLevelPaths> {
                Ok(self.fst.lookup_fd_strvec(s, limit, time_cutoff))
            }

            pub fn lookup_fd_string(
                &self,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> crate::error::Result<HfstOneLevelPaths> {
                Ok(self.fst.lookup_fd_cstr(s, limit, time_cutoff))
            }

            // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-fn]
            pub fn lookup_tokenizer(
                &self,
                tok: &HfstTokenizer,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> crate::error::Result<HfstOneLevelPaths> {
                let sv: StringVector = tok.tokenize_one_level(s, false);
                self.lookup_fd_string_vector(&sv, limit, time_cutoff)
            }

            // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lookup-infinitely-ambiguous-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lookup-infinitely-ambiguous-fn]
            pub fn is_lookup_infinitely_ambiguous_string_vector(&self, s: &StringVector) -> bool {
                self.fst.is_lookup_infinitely_ambiguous_strvec(s)
            }

            pub fn is_lookup_infinitely_ambiguous_string(&self, s: &str) -> bool {
                self.fst.is_lookup_infinitely_ambiguous_str(s)
            }
        }
    };
}

ol_lookup_facade!(WeightedTables);
ol_lookup_facade!(UnweightedTables);

// -----------------------------------------------------------------------------
// extract_nbest helpers.
// -----------------------------------------------------------------------------

// [spec:hfst:def:hfst-transducer.hfst.match-any-n-times-fn]
// [spec:hfst:sem:hfst-transducer.hfst.match-any-n-times-fn]
fn match_any_n_times(n: u32, flags: &crate::hfst_symbol_defs::StringSet) -> String {
    let mut match_any = String::from(" [ ? ");
    for flag in flags.iter() {
        match_any = match_any + "| \"" + flag + "\" ";
    }
    match_any += " ] ";

    let mut match_length = String::from("[");
    for _i in 0..n {
        match_length += &match_any;
    }
    match_length += "]";
    match_length
}

// [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb]
struct ExtractStringsCb_<'a> {
    paths: &'a mut HfstTwoLevelPaths,
    max_num: i32,
}

impl<'a> ExtractStringsCb_<'a> {
    // [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb.extract-strings-cb-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.extract-strings-cb.extract-strings-cb-fn]
    fn new(p: &'a mut HfstTwoLevelPaths, max: i32) -> Self {
        ExtractStringsCb_ {
            paths: p,
            max_num: max,
        }
    }
}

impl<'a> ExtractStringsCb for ExtractStringsCb_<'a> {
    // [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb.operator-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.extract-strings-cb.operator-fn]
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        if is_final {
            self.paths.insert(path.clone());
        }

        RetVal::new(
            (self.max_num < 1) || (self.paths.len() as i32) < self.max_num,
            true,
        )
    }
}
