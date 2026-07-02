//! Port of 'libhfst/src/HfstExtractStrings.h' (header-only) — the path types
//! and the 'ExtractStringsCb' callback interface used by
//! 'HfstTransducer::extract_paths' (the facade consumer is deferred; the
//! callback trait + path types are backend-independent and ported here).

use crate::hfst_data_types::HfstTwoLevelPath;

// [spec:hfst:def:hfst-extract-strings.hfst.string-pair-vector]
pub type StringPairVector = Vec<(String, String)>;
// [spec:hfst:def:hfst-extract-strings.hfst.hfst-two-level-path]
// (== hfst::HfstTwoLevelPath; the canonical struct lives in hfst_data_types)
pub use crate::hfst_data_types::HfstTwoLevelPath as ExtractHfstTwoLevelPath;
// [spec:hfst:def:hfst-extract-strings.hfst.hfst-two-level-paths]
pub use crate::hfst_data_types::HfstTwoLevelPaths as ExtractHfstTwoLevelPaths;

// The WeightedPath / WeightedPaths templates are inside '#ifdef FOO' in the C++
// (never compiled — superseded by HfstOneLevelPath/HfstTwoLevelPath). Ported for
// symbol coverage, kept dead like the original.
// [spec:hfst:def:hfst-extract-strings.hfst.weighted-path]
#[allow(dead_code)]
#[derive(Clone)]
pub struct WeightedPath<W> {
    /* The input string of the path. */
    pub istring: String,
    /* The output string of the path. */
    pub ostring: String,
    /* The weight of the path. */
    pub weight: W,
    /* An optional StringPairVector representation of the path. */
    pub spv: StringPairVector,
    /* Whether the StringPairVector representation is in use. */
    pub is_spv_in_use: bool,
}

#[allow(dead_code)]
impl<W: Clone + PartialEq + PartialOrd + std::ops::Add<Output = W> + std::fmt::Display>
    WeightedPath<W>
{
    // [spec:hfst:def:hfst-extract-strings.hfst.weighted-path.weighted-path-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.weighted-path.weighted-path-fn]
    pub fn new(is: &str, os: &str, w: W) -> Self {
        WeightedPath {
            weight: w,
            istring: is.to_string(),
            ostring: os.to_string(),
            spv: StringPairVector::new(),
            is_spv_in_use: false,
        }
    }

    // [spec:hfst:def:hfst-extract-strings.hfst.weighted-path.operator-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.weighted-path.operator-fn]
    pub fn operator_lt(&self, another: &WeightedPath<W>) -> bool {
        if self.weight == another.weight {
            if self.istring == another.istring {
                if self.ostring == another.ostring {
                    /* Handle here spv. */
                    if !self.is_spv_in_use {
                        return false; /* paths are equivalent */
                    }
                    let common_length = if self.spv.len() < another.spv.len() {
                        self.spv.len()
                    } else {
                        another.spv.len()
                    };
                    /* Go through string pairs. */
                    for i in 0..common_length {
                        if self.spv[i].0 == another.spv[i].0 {
                            if self.spv[i].1 == another.spv[i].1 {
                                continue;
                            }
                            return self.spv[i].1 < another.spv[i].1;
                        }
                        return self.spv[i].0 < another.spv[i].0;
                    }
                    /* Shorter path is smaller. */
                    return self.spv.len() < another.spv.len();
                }
                return self.ostring < another.ostring;
            }
            return self.istring < another.istring;
        }
        self.weight < another.weight
    }

    // [spec:hfst:def:hfst-extract-strings.hfst.weighted-path.to-string-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.weighted-path.to-string-fn]
    pub fn to_string(&self) -> String {
        format!("{}:{}\t{}", self.istring, self.ostring, self.weight)
    }

    pub fn reverse(&mut self) -> &mut Self {
        let mut ib: Vec<u8> = self.istring.clone().into_bytes();
        let n = ib.len();
        for i in 0..(n / 2) {
            ib.swap(i, n - i - 1);
        }
        self.istring = String::from_utf8_lossy(&ib).into_owned();

        let mut ob: Vec<u8> = self.ostring.clone().into_bytes();
        let m = ob.len();
        for i in 0..(m / 2) {
            ob.swap(i, m - i - 1);
        }
        self.ostring = String::from_utf8_lossy(&ob).into_owned();
        self
    }

    pub fn add(&mut self, another: &WeightedPath<W>, in_front: bool) -> &mut Self {
        if in_front {
            self.istring = another.istring.clone() + &self.istring;
            self.ostring = another.ostring.clone() + &self.ostring;
            self.weight = self.weight.clone() + another.weight.clone();
        } else {
            self.istring = self.istring.clone() + &another.istring;
            self.ostring = self.ostring.clone() + &another.ostring;
            self.weight = self.weight.clone() + another.weight.clone();
        }
        self
    }

    pub fn operator_assign(&mut self, another: &WeightedPath<W>) {
        self.istring = another.istring.clone();
        self.ostring = another.ostring.clone();
        self.weight = another.weight.clone();
    }
}

// [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths]
#[allow(dead_code)]
pub struct WeightedPaths<W> {
    _marker: std::marker::PhantomData<W>,
}

#[allow(dead_code)]
impl<W: Clone + PartialEq + PartialOrd + std::ops::Add<Output = W> + std::fmt::Display>
    WeightedPaths<W>
{
    // [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.vector]
    // [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.set]
    // (Vector / Set typedefs -> Vec<WeightedPath<W>>; Set was std::set, but the
    // WeightedPath ordering is via operator_lt — modelled as a Vec here as the
    // C++ Set type alias is unused dead code.)

    // [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.add-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.weighted-paths.add-fn]
    pub fn add_vector_path(v: &mut Vec<WeightedPath<W>>, s: &WeightedPath<W>) {
        for it in v.iter_mut() {
            it.add(s, false);
        }
    }

    pub fn add_path_vector(s: &WeightedPath<W>, v: &mut Vec<WeightedPath<W>>) {
        for it in v.iter_mut() {
            it.add(s, true);
        }
    }

    // [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.cat-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.weighted-paths.cat-fn]
    pub fn cat(v: &mut Vec<WeightedPath<W>>, another_v: &Vec<WeightedPath<W>>) {
        v.extend(another_v.iter().cloned());
    }

    // [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.reverse-strings-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.weighted-paths.reverse-strings-fn]
    pub fn reverse_strings(v: &mut Vec<WeightedPath<W>>) {
        for it in v.iter_mut() {
            it.reverse();
        }
    }
}

// [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.ret-val]
#[derive(Clone, Copy)]
pub struct RetVal {
    pub continueSearch: bool,
    pub continuePath: bool,
}

impl RetVal {
    // [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.ret-val.ret-val-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.extract-strings-cb.ret-val.ret-val-fn]
    pub fn new(s: bool, p: bool) -> Self {
        RetVal {
            continueSearch: s,
            continuePath: p,
        }
    }

    // [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.ret-val.operator-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.extract-strings-cb.ret-val.operator-fn]
    pub fn operator_assign(&mut self, o: &RetVal) {
        self.continueSearch = o.continueSearch;
        self.continuePath = o.continuePath;
    }
}

// [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb]
// The abstract callback (pure virtual operator()) becomes a trait.
pub trait ExtractStringsCb {
    // This function is called by extract_paths after every transition with the
    // path up to that point, and whether or not the path ends at a final state.
    // [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.operator-fn]
    // [spec:hfst:sem:hfst-extract-strings.hfst.extract-strings-cb.operator-fn]
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal;
}
