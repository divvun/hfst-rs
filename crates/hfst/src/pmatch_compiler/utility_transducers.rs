//! Cached character-class acceptors and the casing transducers built on them.

use super::*;

// [spec:hfst:def:pmatch-utils.hfst.pmatch.acceptor-from-cstr-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.acceptor-from-cstr-fn]
pub fn acceptor_from_cstr<B: AlgebraBackend>(
    strings: &[&str],
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval: HfstTransducer<B> = HfstTransducer::new();
    let mut i = 0;
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.array-len-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.array-len-fn]
    while i < strings.len() {
        let tmp = HfstTransducer::new_symbol(strings[i])?;
        retval.disjunct(&tmp, true)?;
        i += 1;
    }
    retval.minimize()?;
    Ok(retval)
}

impl<B: AlgebraBackend> PmatchUtilityTransducers<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    pub fn new() -> crate::error::Result<PmatchUtilityTransducers<B>> {
        let mut retval = PmatchUtilityTransducers {
            latin1_acceptor: PmatchUtilityTransducers::make_latin1_acceptor()?,
            latin1_alpha_acceptor: PmatchUtilityTransducers::make_latin1_alpha_acceptor()?,
            latin1_lowercase_acceptor: PmatchUtilityTransducers::make_latin1_lowercase_acceptor()?,
            latin1_uppercase_acceptor: PmatchUtilityTransducers::make_latin1_uppercase_acceptor()?,
            combining_accent_acceptor: PmatchUtilityTransducers::make_combining_accent_acceptor()?,
            latin1_numeral_acceptor: PmatchUtilityTransducers::make_latin1_numeral_acceptor()?,
            latin1_punct_acceptor: PmatchUtilityTransducers::make_latin1_punct_acceptor()?,
            latin1_whitespace_acceptor: PmatchUtilityTransducers::make_latin1_whitespace_acceptor(
            )?,
            capify: HfstTransducer::new(),
            lowerfy: HfstTransducer::new(),
        };
        retval.lowerfy = retval.make_lowerfy()?;
        retval.capify = retval.make_capify()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-acceptor-fn]
    pub fn make_latin1_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = PmatchUtilityTransducers::make_latin1_alpha_acceptor()?;
        let mut tmp: HfstTransducer<B> = PmatchUtilityTransducers::make_latin1_numeral_acceptor()?;
        retval.disjunct(&tmp, true)?;
        tmp = PmatchUtilityTransducers::make_latin1_punct_acceptor()?;
        retval.disjunct(&tmp, true)?;
        tmp = PmatchUtilityTransducers::make_latin1_whitespace_acceptor()?;
        retval.disjunct(&tmp, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    pub fn make_latin1_alpha_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> =
            PmatchUtilityTransducers::make_latin1_lowercase_acceptor()?;
        let tmp: HfstTransducer<B> = PmatchUtilityTransducers::make_latin1_uppercase_acceptor()?;
        retval.disjunct(&tmp, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    pub fn make_latin1_lowercase_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = acceptor_from_cstr(latin1_lower)?;
        let tmp: HfstTransducer<B> = PmatchUtilityTransducers::make_combining_accent_acceptor()?;
        retval.disjunct(&tmp, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    pub fn make_latin1_uppercase_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = acceptor_from_cstr(latin1_upper)?;
        let tmp: HfstTransducer<B> = PmatchUtilityTransducers::make_combining_accent_acceptor()?;
        retval.disjunct(&tmp, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    pub fn make_combining_accent_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        acceptor_from_cstr(combining_accents)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    pub fn make_latin1_numeral_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        let num: String = "0123456789".to_string();
        for it in num.chars() {
            retval.disjunct(&HfstTransducer::new_symbol(&it.to_string())?, true)?;
        }
        // retval->minimize(); ?
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    pub fn make_latin1_punct_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        acceptor_from_cstr(latin1_punct)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    pub fn make_latin1_whitespace_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        acceptor_from_cstr(latin1_whitespace)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-capify-fn]
    pub fn make_capify(&mut self) -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut i: usize = 0;
        while i < latin1_upper.len() {
            retval.disjunct(
                &HfstTransducer::new_tokenized_pair(latin1_lower[i], latin1_upper[i], &tok)?,
                true,
            )?;
            i += 1;
        }
        let mut accents: HfstTransducer<B> =
            HfstTransducer::new_copy(&self.combining_accent_acceptor)?;
        accents.optionalize()?;
        retval.concatenate(&accents, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-lowerfy-fn]
    pub fn make_lowerfy(&mut self) -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut i: usize = 0;
        while i < latin1_upper.len() {
            retval.disjunct(
                &HfstTransducer::new_tokenized_pair(latin1_upper[i], latin1_lower[i], &tok)?,
                true,
            )?;
            i += 1;
        }
        let mut accents: HfstTransducer<B> =
            HfstTransducer::new_copy(&self.combining_accent_acceptor)?;
        accents.optionalize()?;
        retval.concatenate(&accents, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    pub fn get_lowercase_acceptor_from_transducer(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut lowercase: HfstTransducer<B> = HfstTransducer::new();
        let ss: StringSet = t.get_alphabet()?;
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1
                && icu::properties::CodePointSetData::new::<icu::properties::props::Lowercase>()
                    .contains(us[0])
            {
                lowercase.disjunct(&HfstTransducer::new_symbol(it)?, true)?;
            }
        }
        Ok(lowercase)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    pub fn get_uppercase_acceptor_from_transducer(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut uppercase: HfstTransducer<B> = HfstTransducer::new();
        let ss: StringSet = t.get_alphabet()?;
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1
                && icu::properties::CodePointSetData::new::<icu::properties::props::Uppercase>()
                    .contains(us[0])
            {
                uppercase.disjunct(&HfstTransducer::new_symbol(it)?, true)?;
            }
        }
        Ok(uppercase)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    pub fn lowercaser_from_transducer(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut lowercase: HfstTransducer<B> = HfstTransducer::new();
        let ss: StringSet = t.get_alphabet()?;
        let mut uppercases_seen: StringSet = StringSet::new();
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1 {
                let this_unichar: char = us[0];
                if icu::properties::CodePointSetData::new::<icu::properties::props::Alphabetic>()
                    .contains(this_unichar)
                {
                    let upper: String = icu::casemap::CaseMapper::new()
                        .uppercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    if uppercases_seen.contains(upper.as_str()) {
                        continue;
                    }
                    uppercases_seen.insert(Symbol::from(upper.clone()));
                    let lower: String = icu::casemap::CaseMapper::new()
                        .lowercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    lowercase.disjunct(&HfstTransducer::new_symbol_pair(&upper, &lower)?, true)?;
                }
            }
        }
        Ok(lowercase)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    pub fn uppercaser_from_transducer(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut uppercase: HfstTransducer<B> = HfstTransducer::new();
        let ss: StringSet = t.get_alphabet()?;
        let mut uppercases_seen: StringSet = StringSet::new();
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1 {
                let this_unichar: char = us[0];
                if icu::properties::CodePointSetData::new::<icu::properties::props::Alphabetic>()
                    .contains(this_unichar)
                {
                    let upper: String = icu::casemap::CaseMapper::new()
                        .uppercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    if uppercases_seen.contains(upper.as_str()) {
                        continue;
                    }
                    uppercases_seen.insert(Symbol::from(upper.clone()));
                    let lower: String = icu::casemap::CaseMapper::new()
                        .lowercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    uppercase.disjunct(&HfstTransducer::new_symbol_pair(&lower, &upper)?, true)?;
                }
            }
        }
        Ok(uppercase)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.cap-fn]
    pub fn cap(
        &mut self,
        t: &HfstTransducer<B>,
        side: Side,
        optional: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // This is to match flags in t with ?'s in "anything": these composes run
        // with Xerox-style composition enabled.
        let cfg = crate::hfst_transducer::EngineConfig {
            xerox_composition: true,
            ..Default::default()
        };

        let mut retval: HfstTransducer<B>;
        let mut cap: HfstTransducer<B> = self.uppercaser_from_transducer(t)?;
        let mut decap: HfstTransducer<B> = HfstTransducer::new_copy(&cap)?;
        decap.invert()?;
        let mut anything: HfstTransducer<B> = HfstTransducer::identity_pair();
        let mut anything_but_whitespace_star: HfstTransducer<B> =
            HfstTransducer::new_copy(&anything)?;
        anything_but_whitespace_star.subtract(&self.latin1_whitespace_acceptor, true)?;
        anything_but_whitespace_star.repeat_star()?;
        if !optional {
            // don't let lowercased first letters through
            anything.subtract(&self.get_lowercase_acceptor_from_transducer(t)?, true)?;
        }
        // As in the regexp
        // [[[["A":"a" [[\" "]* (" " "A":"a")]* ] .o. [{ab ad}:{ef eh}].u]] .o.
        //   [{ab ad}:{ef eh}] ] .o. [[{ab ad}:{ef eh}].l] .o.
        //   ["e":"E" [[\" "]+ (" " "e":"E")]*]
        if side == Side::Lower {
            retval = HfstTransducer::new_copy(t)?;
            cap.disjunct(&anything, true)?;
            // Cap is the first letter to either capitalize or accept if it's not a
            // lowercase letter
            let mut continuation: HfstTransducer<B> =
                HfstTransducer::new_copy(&anything_but_whitespace_star)?;
            // continuation is the rest of the first word
            let mut more_caps: HfstTransducer<B> =
                HfstTransducer::new_copy(&self.latin1_whitespace_acceptor)?;
            // more_caps is more words to capitalize
            more_caps.concatenate(&cap, true)?;
            more_caps.optionalize()?;
            continuation.concatenate(&more_caps, true)?;
            continuation.repeat_star()?;
            cap.concatenate(&continuation, true)?;
            retval.compose_with_config(&cap, true, &cfg)?;
        } else if side == Side::Upper {
            decap.disjunct(&anything, true)?;
            let mut continuation: HfstTransducer<B> =
                HfstTransducer::new_copy(&anything_but_whitespace_star)?;
            let mut more_decaps: HfstTransducer<B> =
                HfstTransducer::new_copy(&self.latin1_whitespace_acceptor)?;
            more_decaps.concatenate(&decap, true)?;
            more_decaps.optionalize()?;
            continuation.concatenate(&more_decaps, true)?;
            continuation.repeat_star()?;
            retval = HfstTransducer::new_copy(&decap)?;
            retval.concatenate(&continuation, true)?;
            retval.compose_with_config(t, true, &cfg)?;
        } else {
            // both
            decap.disjunct(&anything, true)?;
            let mut continuation: HfstTransducer<B> =
                HfstTransducer::new_copy(&anything_but_whitespace_star)?;
            let mut more_decaps: HfstTransducer<B> =
                HfstTransducer::new_copy(&self.latin1_whitespace_acceptor)?;
            more_decaps.concatenate(&decap, true)?;
            more_decaps.optionalize()?;
            continuation.concatenate(&more_decaps, true)?;
            continuation.repeat_star()?;
            retval = HfstTransducer::new_copy(&decap)?;
            retval.concatenate(&continuation, true)?;
            retval.compose_with_config(t, true, &cfg)?;
            let mut continuation2: HfstTransducer<B> =
                HfstTransducer::new_copy(&anything_but_whitespace_star)?;
            let mut more_caps: HfstTransducer<B> =
                HfstTransducer::new_copy(&self.latin1_whitespace_acceptor)?;
            cap.disjunct(&anything, true)?;
            more_caps.concatenate(&cap, true)?;
            more_caps.optionalize()?;
            continuation2.concatenate(&more_caps, true)?;
            continuation2.repeat_star()?;
            cap.concatenate(&continuation2, true)?;
            retval.compose_with_config(&cap, true, &cfg)?;
            retval.output_project()?;
        }
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.tolower-fn]
    pub fn tolower(
        &mut self,
        t: &HfstTransducer<B>,
        side: Side,
        optional: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // This is to match flags in t with ?'s in "anything": these composes run
        // with Xerox-style composition enabled.
        let cfg = crate::hfst_transducer::EngineConfig {
            xerox_composition: true,
            ..Default::default()
        };

        let mut anything: HfstTransducer<B> =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        if !optional {
            anything.subtract(&self.get_uppercase_acceptor_from_transducer(t)?, true)?;
        }
        let mut retval: HfstTransducer<B>;
        if side == Side::Lower {
            let mut lowercase: HfstTransducer<B> = self.lowercaser_from_transducer(t)?;
            lowercase.disjunct(&anything, true)?;
            lowercase.repeat_star()?;
            retval = HfstTransducer::new_copy(t)?;
            retval.compose_with_config(&lowercase, true, &cfg)?;
        } else if side == Side::Upper {
            retval = self.uppercaser_from_transducer(t)?;
            retval.disjunct(&anything, true)?;
            retval.repeat_star()?;
            retval.compose_with_config(t, true, &cfg)?;
        } else {
            // both
            retval = self.uppercaser_from_transducer(t)?;
            retval.disjunct(&anything, true)?;
            retval.repeat_star()?;
            retval.compose_with_config(t, true, &cfg)?;
            let mut lowercase: HfstTransducer<B> = self.lowercaser_from_transducer(t)?;
            lowercase.disjunct(&anything, true)?;
            lowercase.repeat_star()?;
            retval.compose_with_config(&lowercase, true, &cfg)?;
        }
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.toupper-fn]
    pub fn toupper(
        &mut self,
        t: &HfstTransducer<B>,
        side: Side,
        optional: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // This is to match flags in t with ?'s in "anything": these composes run
        // with Xerox-style composition enabled.
        let cfg = crate::hfst_transducer::EngineConfig {
            xerox_composition: true,
            ..Default::default()
        };

        let mut anything: HfstTransducer<B> =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        if !optional {
            anything.subtract(&self.get_lowercase_acceptor_from_transducer(t)?, true)?;
        }
        let mut retval: HfstTransducer<B>;
        if side == Side::Lower {
            let mut uppercase: HfstTransducer<B> = self.uppercaser_from_transducer(t)?;
            uppercase.disjunct(&anything, true)?;
            uppercase.repeat_star()?;
            retval = HfstTransducer::new_copy(t)?;
            retval.compose_with_config(&uppercase, true, &cfg)?;
        } else if side == Side::Upper {
            retval = self.lowercaser_from_transducer(t)?;
            retval.disjunct(&anything, true)?;
            retval.repeat_star()?;
            retval.compose_with_config(t, true, &cfg)?;
        } else {
            // both
            retval = self.lowercaser_from_transducer(t)?;
            retval.disjunct(&anything, true)?;
            retval.repeat_star()?;
            retval.compose_with_config(t, true, &cfg)?;
            let mut uppercase: HfstTransducer<B> = self.uppercaser_from_transducer(t)?;
            uppercase.disjunct(&anything, true)?;
            uppercase.repeat_star()?;
            retval.compose_with_config(&uppercase, true, &cfg)?;
        }
        retval.minimize()?;
        Ok(retval)
    }
}
