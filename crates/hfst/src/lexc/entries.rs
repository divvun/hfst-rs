//! The incremental registration API: alphabets, lexicon names, and the
//! string, string-pair and regex entries accumulated into the trie.

use super::recode::{flag_joiner_encode, joiner_encode, reg_expresion_encode, replace_zero};
use super::*;

// 'EPSILON_' and the med-alignment direction codes back 'find_med_alingment'.
const EPSILON_: &str = "@@ANOTHER_EPSILON@@";
const SUBSTITUTE: u32 = 2; // diag
const DELETE: u32 = 1; // left
const INSERT: u32 = 0; // down

// [spec:hfst:def:lexc-utils.hfst.lexc.find-med-alingment-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.find-med-alingment-fn]
fn find_med_alingment(s1: &[String], s2: &[String]) -> (Vec<String>, Vec<String>) {
    let substitution: i64 = 100;
    let deletion: i64 = 1;
    let insertion: i64 = 1;

    let len1 = s1.len();
    let len2 = s2.len();
    let mut d = vec![vec![0u32; len2 + 1]; len1 + 1];
    let mut dir = vec![vec![0u32; len2 + 1]; len1 + 1];
    d[0][0] = 0;
    dir[0][0] = 0;
    for i in 1..=len1 {
        d[i][0] = (deletion * i as i64) as u32;
        dir[i][0] = DELETE;
    }
    for i in 1..=len2 {
        d[0][i] = (insertion * i as i64) as u32;
        dir[0][i] = INSERT;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let sub = d[i - 1][j - 1] as i64
                + (if s1[i - 1] == s2[j - 1] {
                    0
                } else {
                    substitution
                });
            let ins = d[i][j - 1] as i64 + insertion;
            let del = d[i - 1][j] as i64 + deletion;

            if sub <= ins && sub <= del {
                d[i][j] = sub as u32;
                dir[i][j] = SUBSTITUTE;
            }
            // Prioritise "del" over "ins" on ties so the first string gets its
            // zeroes before the second one (matches the C++ comment).
            else if del <= sub && del <= ins {
                d[i][j] = del as u32;
                dir[i][j] = DELETE;
            } else {
                d[i][j] = ins as u32;
                dir[i][j] = INSERT;
            }
        }
    }

    let mut medcwordin: Vec<String> = Vec::new();
    let mut medcwordout: Vec<String> = Vec::new();

    let mut x = i32::try_from(s1.len()).expect("value out of i32 range");
    let mut y = i32::try_from(s2.len()).expect("value out of i32 range");
    while x > 0 || y > 0 {
        let dir_value = dir[x as usize][y as usize];

        if dir_value == SUBSTITUTE {
            medcwordin.push(s1[(x - 1) as usize].clone());
            medcwordout.push(s2[(y - 1) as usize].clone());
            x -= 1;
            y -= 1;
        } else if dir_value == INSERT {
            medcwordin.push(EPSILON_.to_string());
            medcwordout.push(s2[(y - 1) as usize].clone());
            y -= 1;
        } else {
            medcwordin.push(s1[(x - 1) as usize].clone());
            medcwordout.push(EPSILON_.to_string());
            x -= 1;
        }
    }

    medcwordin.reverse();
    medcwordout.reverse();
    (medcwordin, medcwordout)
}

impl<B: AlgebraBackend> LexcCompiler<B> {
    // ----- incremental registration API -----

    pub fn add_no_flag(&mut self, lexname: &str) -> &mut Self {
        self.noFlags_.insert(Symbol::new(lexname));
        self
    }

    pub fn add_alphabet(&mut self, alpha: &str) -> &mut Self {
        // nfst-lexc preserves an escaped literal zero as `@ZERO@` so it stays
        // distinct from bare `0` (epsilon) during tokenization. Entries are
        // decoded back to the literal symbol before the missing-alphabet
        // check, so declarations must use the same decoded spelling there.
        self.alphabets.insert(Symbol::from(replace_zero(alpha)));
        self.tokenizer.add_multichar_symbol(alpha);
        if !self.quiet && self.verbose {
            // warn about undefined multichars
            self.xre.add_defined_multichar_symbol(alpha);
        }
        self
    }

    /// Construct vector 'nameJoiner data contJoiner' and add it to the trie.
    pub fn add_string_entry(&mut self, data: &str, continuation: &str, weight: f64) -> &mut Self {
        self.currentEntries_ += 1;
        self.totalEntries_ += 1;
        self.unicode_check(data);
        self.continuations.insert(Symbol::new(continuation));
        let encoded_cont = if self.with_flags {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer.add_multichar_symbol(&encoded_cont);

        // build string pair vector map
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags {
            if !self.noFlags_.contains(cur.as_str()) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer.add_multichar_symbol(&joiner_enc);
        self.tokenizer.add_multichar_symbol("0"); // epsilon
        self.tokenizer.add_multichar_symbol("@ZERO@"); // literal zero
        let mut new_vector = self.tokenizer.tokenize(
            &format!("{}{}{}", joiner_enc, data, encoded_cont),
            self.split_characters,
        );
        // "0"      -> "@0@"  (single symbols)
        // "@ZERO@" -> "0"    (everywhere)
        let mut i = 0;
        while i < new_vector.len() {
            if new_vector[i].0 == "0" {
                new_vector[i].0 = Symbol::new_static("@0@");
            }
            if let Some(start_pos) = new_vector[i].0.find("@ZERO@") {
                let mut s = new_vector[i].0.to_string();
                s.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
                new_vector[i].0 = Symbol::from(s);
            }
            if new_vector[i].1 == "0" {
                new_vector[i].1 = Symbol::new_static("@0@");
            }
            if let Some(start_pos) = new_vector[i].1.find("@ZERO@") {
                let mut s = new_vector[i].1.to_string();
                s.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
                new_vector[i].1 = Symbol::from(s);
            }
            let first = new_vector[i].0.clone();
            if !self.alphabets.contains(first.as_str()) {
                if first.starts_with('@') && first.ends_with('@') {
                    i += 1;
                    continue;
                }
                if first.starts_with('$') && first.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets {
                    let message = format!("Adding {first} to Alphabets [-Wmissing-alphabets]");
                    if first == "0" {
                        crate::diag::emit(
                            &self.source_name,
                            &self.source,
                            self.current_span.clone(),
                            crate::diag::Severity::Info,
                            &message,
                        );
                    } else if self.treat_warnings_as_errors {
                        self.error_at_current_token(&message);
                        self.parseErrors_ = true;
                    } else {
                        self.warning_at_current_token(&message);
                    }
                }
                self.add_alphabet(&first);
            }
            i += 1;
        }
        let w = weight as f32;
        self.stringsTrie_.disjunct_path(&new_vector, w);
        self
    }

    // callback function to stuff so static and uses global singleton :-(
    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.warn-about-one-sided-flags-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.warn-about-one-sided-flags-fn]
    /// In the C++ this was a static tokenize callback reaching the 'lexc'
    /// global; the re-entrant port applies it to the tokenized pairs directly.
    fn warn_about_one_sided_flags(&mut self, symbol_pair: &StringPair) {
        if crate::hfst_flag_diacritics::FdOperation::is_diacritic(&symbol_pair.0) {
            if symbol_pair.0 != symbol_pair.1 {
                let errm = format!(
                    "one-sided flag diacritic {}:{} [-Wone-sided-flags]",
                    symbol_pair.0, symbol_pair.1
                );
                if self.warn_one_sided_flags && self.treat_warnings_as_errors {
                    self.error_at_current_token(&errm);
                    self.parseErrors_ = true;
                }
                if self.warn_one_sided_flags {
                    self.warning_at_current_token(&errm);
                }
            }
        } else if crate::hfst_flag_diacritics::FdOperation::is_diacritic(&symbol_pair.1) {
            let errm = format!(
                "one-sided flag diacritic {}:{} [-Wone-sided-flags]",
                symbol_pair.0, symbol_pair.1
            );
            if self.warn_one_sided_flags && self.treat_warnings_as_errors {
                self.error_at_current_token(&errm);
                self.parseErrors_ = true;
            }
            if self.warn_one_sided_flags {
                self.warning_at_current_token(&errm);
            }
        }
    }

    pub fn add_string_pair_entry(
        &mut self,
        upper: &str,
        lower: &str,
        continuation: &str,
        weight: f64,
    ) -> &mut Self {
        self.currentEntries_ += 1;
        self.totalEntries_ += 1;
        self.unicode_check(upper);
        self.unicode_check(lower);
        self.continuations.insert(Symbol::new(continuation));
        let encoded_cont = if self.with_flags {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer.add_multichar_symbol(&encoded_cont);

        // build string pair vector map
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags {
            if !self.noFlags_.contains(cur.as_str()) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer.add_multichar_symbol(&joiner_enc);
        self.tokenizer.add_multichar_symbol("0"); // epsilon
        self.tokenizer.add_multichar_symbol("@ZERO@"); // literal zero

        let mut new_vector: StringPairVector;

        if self.align_strings {
            let tmp = self
                .tokenizer
                .tokenize_pair(upper, lower, self.split_characters);
            let mut one: Vec<String> = Vec::new();
            let mut two: Vec<String> = Vec::new();
            for it in &tmp {
                if it.0 != "@_EPSILON_SYMBOL_@" {
                    one.push(it.0.to_string());
                }
                if it.1 != "@_EPSILON_SYMBOL_@" {
                    two.push(it.1.to_string());
                }
            }

            let med_vectors = find_med_alingment(&one, &two);
            let as1: String = med_vectors.0.concat();
            let as2: String = med_vectors.1.concat();

            new_vector = self.tokenizer.tokenize_pair(
                &format!("{}{}{}", joiner_enc, as1, encoded_cont),
                &format!("{}{}{}", joiner_enc, as2, encoded_cont),
                self.split_characters,
            );
        } else {
            let upper_v = self.tokenizer.tokenize(upper, self.split_characters);
            let lower_v = self.tokenizer.tokenize(lower, self.split_characters);

            let upper_size = i32::try_from(upper_v.len()).expect("value out of i32 range");
            let lower_size = i32::try_from(lower_v.len()).expect("value out of i32 range");

            if upper_size > lower_size {
                let mut epsilons = String::new();
                for _ in 1..=(upper_size - lower_size) {
                    epsilons.push_str("@@ANOTHER_EPSILON@@");
                }
                new_vector = self.tokenizer.tokenize_pair(
                    &format!("{}{}{}", joiner_enc, upper, encoded_cont),
                    &format!("{}{}{}{}", joiner_enc, lower, epsilons, encoded_cont),
                    self.split_characters,
                );
            } else if upper_size < lower_size {
                let mut epsilons = String::new();
                for _ in 1..=(lower_size - upper_size) {
                    epsilons.push_str("@@ANOTHER_EPSILON@@");
                }
                new_vector = self.tokenizer.tokenize_pair(
                    &format!("{}{}{}{}", joiner_enc, upper, epsilons, encoded_cont),
                    &format!("{}{}{}", joiner_enc, lower, encoded_cont),
                    self.split_characters,
                );
            } else {
                new_vector = self.tokenizer.tokenize_pair(
                    &format!("{}{}{}", joiner_enc, upper, encoded_cont),
                    &format!("{}{}{}", joiner_enc, lower, encoded_cont),
                    self.split_characters,
                );
            }
        }

        // The C++ passed 'warn_about_one_sided_flags' as the tokenize callback;
        // the re-entrant port tokenizes plainly then applies it to each pair.
        for sp in new_vector.iter() {
            let sp = sp.clone();
            self.warn_about_one_sided_flags(&sp);
        }

        let mut i = 0;
        while i < new_vector.len() {
            if new_vector[i].0 == "0" {
                new_vector[i].0 = Symbol::new_static("@0@");
            }
            if let Some(start_pos) = new_vector[i].0.find("@ZERO@") {
                let mut s = new_vector[i].0.to_string();
                s.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
                new_vector[i].0 = Symbol::from(s);
            }
            if new_vector[i].1 == "0" {
                new_vector[i].1 = Symbol::new_static("@0@");
            }
            if let Some(start_pos) = new_vector[i].1.find("@ZERO@") {
                let mut s = new_vector[i].1.to_string();
                s.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
                new_vector[i].1 = Symbol::from(s);
            }
            let first = new_vector[i].0.clone();
            if !self.alphabets.contains(first.as_str()) {
                if first.starts_with('@') && first.ends_with('@') {
                    i += 1;
                    continue;
                }
                if first.starts_with('$') && first.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets {
                    let message = format!("Adding {first} to Alphabets [-Wmissing-alphabets]");
                    if first == "0" {
                        crate::diag::emit(
                            &self.source_name,
                            &self.source,
                            self.current_span.clone(),
                            crate::diag::Severity::Info,
                            &message,
                        );
                    } else if self.treat_warnings_as_errors {
                        self.error_at_current_token(&message);
                        self.parseErrors_ = true;
                    } else {
                        self.warning_at_current_token(&message);
                    }
                }
                self.add_alphabet(&first);
            }
            let second = new_vector[i].1.clone();
            if !self.alphabets.contains(second.as_str()) {
                if second.starts_with('@') && second.ends_with('@') {
                    i += 1;
                    continue;
                }
                if second.starts_with('$') && second.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets {
                    let message = format!("Adding {second} to Alphabets [-Wmissing-alphabets]");
                    if second == "0" {
                        crate::diag::emit(
                            &self.source_name,
                            &self.source,
                            self.current_span.clone(),
                            crate::diag::Severity::Info,
                            &message,
                        );
                    } else if self.treat_warnings_as_errors {
                        self.error_at_current_token(&message);
                        self.parseErrors_ = true;
                    } else {
                        self.warning_at_current_token(&message);
                    }
                }
                self.add_alphabet(&second);
            }
            i += 1;
        }

        let w = weight as f32;
        self.stringsTrie_.disjunct_path(&new_vector, w);
        self
    }

    /// Construct transducer 'nameJoiner XRE contJoiner' and add it to the trie.
    pub fn add_xre_entry(
        &mut self,
        regexp: &str,
        continuation: &str,
        weight: f64,
    ) -> crate::error::Result<&mut Self> {
        self.currentEntries_ += 1;
        self.totalEntries_ += 1;
        self.continuations.insert(Symbol::new(continuation));
        let encoded_cont = if self.with_flags {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer.add_multichar_symbol(&encoded_cont);

        let Some(mut new_paths) = self.xre.compile(regexp) else {
            self.error_at_current_token("Unable to parse regular expression");
            self.parseErrors_ = true;
            return Ok(self);
        };
        new_paths.optimize()?;
        let new_alphabets = new_paths.get_alphabet()?;
        for new_alpha in &new_alphabets {
            if self.alphabets.contains(new_alpha.as_str()) {
                continue;
            }
            if matches!(
                new_alpha.as_str(),
                "@_EPSILON_SYMBOL_@" | "@_UNKNOWN_SYMBOL_@" | "@_IDENTITY_SYMBOL_@"
            ) {
                continue;
            }
            let errm = format!(
                "implicit Alphabet {} in regex [-Wmissing-alphabets]",
                new_alpha
            );
            if new_alpha.chars().count() > 1 {
                self.warning_at_current_token(&errm);
                warn!("you shoudl add {} to Multichar_Symbols section", new_alpha);
            } else if self.warn_missing_alphabets && self.treat_warnings_as_errors {
                self.error_at_current_token(&errm);
                self.parseErrors_ = true;
            } else if self.warn_missing_alphabets {
                self.warning_at_current_token(&errm);
            }
            self.add_alphabet(new_alpha);
        }

        // encode key; keep regexps with different continuations separate
        let mut regex_key = format!("{}_{}", self.currentLexiconName_, continuation);
        regex_key = reg_expresion_encode(&regex_key);
        self.tokenizer.add_multichar_symbol(&regex_key);

        let entry = match self.regexps.entry(Symbol::from(regex_key.clone())) {
            std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::btree_map::Entry::Vacant(e) => e.insert(HfstTransducer::new()),
        };
        entry.disjunct(&new_paths, true)?.optimize()?;

        if !self.quiet && self.currentEntries_.is_multiple_of(10000) {
            info!("{}...", self.currentEntries_);
        }

        // add key to trie
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags {
            if !self.noFlags_.contains(cur.as_str()) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer.add_multichar_symbol(&joiner_enc);
        let new_vector = self.tokenizer.tokenize(
            &format!("{}{}{}", joiner_enc, regex_key, encoded_cont),
            false,
        );
        let w = weight as f32;
        self.stringsTrie_.disjunct_path(&new_vector, w);
        Ok(self)
    }

    pub fn add_xre_definition(&mut self, definition_name: &str, xre: &str) -> &mut Self {
        // FIXME: collect implicit characters
        self.xre.define(definition_name, xre);
        if !self.quiet {
            info!(
                "Defined '{}': ? Kb., ? states, ? arcs, ? paths.",
                definition_name
            );
        }
        self
    }

    pub fn set_current_lexicon_name(&mut self, lexicon_name: &str) -> &mut Self {
        self.currentLexiconName_ = lexicon_name.to_string();

        if self.lexiconNames_.contains(lexicon_name) {
            if !self.warn_repeated_lexicons && self.treat_warnings_as_errors {
                self.error_at_current_token(
                    "Lexicon is defined more than once! [-Wrepeated-lexicons]",
                );
                self.parseErrors_ = true;
            } else if self.warn_repeated_lexicons {
                self.warning_at_current_token(
                    "Lexicon is defined more than once! [-Wrepeated-lexicons]",
                );
            }
        }

        self.lexiconNames_.insert(Symbol::new(lexicon_name));
        if !self.noFlags_.contains(lexicon_name) {
            // [spec:hfst:def:lexc-compiler.hfst.lexc.encoded-name-fn]
            // [spec:hfst:sem:lexc-compiler.hfst.lexc.encoded-name-fn]
            // NOTE: faithful to the C++, the second encode is applied to the
            // already-'$P'-encoded string (flagJoinerEncode mutated in place).
            let mut encoded_name = flag_joiner_encode(lexicon_name, false);
            self.tokenizer.add_multichar_symbol(&encoded_name);
            encoded_name = flag_joiner_encode(&encoded_name, true);
            self.tokenizer.add_multichar_symbol(&encoded_name);
        } else {
            let encoded_name = joiner_encode(lexicon_name);
            self.tokenizer.add_multichar_symbol(&encoded_name);
        }

        if self.first_lexicon && lexicon_name == "Root" {
            self.set_initial_lexicon_name(lexicon_name);
        } else if self.first_lexicon && lexicon_name != "Root" {
            if self.treat_warnings_as_errors {
                self.error_at_current_token("first lexicon is not named Root");
                self.parseErrors_ = true;
            } else {
                self.warning_at_current_token("first lexicon is not named Root");
            }
            self.set_initial_lexicon_name(lexicon_name);
        } else if !self.first_lexicon && lexicon_name == "Root" {
            if self.treat_warnings_as_errors {
                self.error_at_current_token("Root is not first the first lexicon");
                self.parseErrors_ = true;
            } else {
                self.warning_at_current_token("Root is not first the first lexicon");
            }
            self.set_initial_lexicon_name(lexicon_name);
        }
        if !self.quiet {
            let mut line = String::new();
            if !self.first_lexicon {
                line.push_str(&format!("{} ", self.currentEntries_));
            }
            line.push_str(&format!("{}...", lexicon_name));
            info!("{}", line);
        }
        self.first_lexicon = false;

        self.currentEntries_ = 0;
        self
    }

    pub fn set_initial_lexicon_name(&mut self, lexicon_name: &str) -> &mut Self {
        self.initialLexiconName_ = lexicon_name.to_string();
        self.lexiconNames_.insert(Symbol::new(lexicon_name));
        // for connectedness calculation:
        self.continuations.insert(Symbol::new(lexicon_name));
        self
    }
}
