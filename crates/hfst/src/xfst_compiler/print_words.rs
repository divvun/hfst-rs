//! The print commands that list paths: words, random words, and the
//! shortest and longest strings.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-print-symbol-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-print-symbol-fn]
    // @brief Get the print symbol for \a symbol.
    // @see print_flags
    fn get_print_symbol(&mut self, symbol: &str) -> String {
        if self.variables["show-flags"] == "OFF" && // show no flags
            crate::hfst_flag_diacritics::FdOperation::is_diacritic(symbol)
        // symbol is flag
        {
            return String::new(); // print nothing
        }
        if crate::hfst_symbol_defs::internal_epsilon == symbol {
            return String::new();
        }
        if crate::hfst_symbol_defs::internal_unknown == symbol
            || crate::hfst_symbol_defs::internal_identity == symbol
        {
            return "?".to_string();
        }
        symbol.to_string()
    }

    pub fn shortest_string(
        &mut self,
        transducer: &HfstTransducer<B>,
        paths: &mut HfstTwoLevelPaths,
    ) -> crate::error::Result<&mut Self> {
        transducer.extract_shortest_paths(paths)?;
        Ok(self)
    }

    // @brief Print shortest string of network
    pub fn print_shortest_string(
        &mut self,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        let mut paths = HfstTwoLevelPaths::new();
        let net = self.net(topmost).clone();
        self.shortest_string(&net, &mut paths)?;

        if paths.is_empty() {
            println!("transducer is empty");
        } else {
            self.print_paths_two(&paths, oss, -1);
        }
        self.flush();
        self.prompt();
        Ok(self)
    }

    // @brief Print length of shortest string
    pub fn print_shortest_string_size(
        &mut self,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        let mut paths = HfstTwoLevelPaths::new();
        let net = self.net(topmost).clone();
        self.shortest_string(&net, &mut paths)?;

        if paths.is_empty() {
            println!("transducer is empty");
        } else {
            let _ = writeln!(
                oss,
                "{}",
                paths
                    .iter()
                    .next()
                    .expect("paths non-empty, checked above")
                    .second
                    .len() as i32
            );
        }
        self.flush();
        self.prompt();
        Ok(self)
    }

    // @brief Print longest string in network
    pub fn print_longest_string(
        &mut self,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        self.print_longest_string_or_its_size(oss, false)
    }

    // @brief Print length of longest string
    pub fn print_longest_string_size(
        &mut self,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        self.print_longest_string_or_its_size(oss, true)
    }

    // @brief Print strings of lower language
    pub fn print_lower_words(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        self.print_words_level(name, number, oss, Level::LOWER_LEVEL)
    }

    // @brief Print random strings of lower language
    pub fn print_random_lower(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        let mut paths = HfstTwoLevelPaths::new();

        // [spec:hfst:def:xfst-compiler.hfst.xfst.tmp-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.tmp-fn]
        let mut tmp: HfstTransducer<B> = HfstTransducer::new();
        if name.is_empty() {
            let Some(temp) = self.top() else {
                return Ok(self);
            };
            tmp = HfstTransducer::new_from_transducer(self.net(temp));
        } else {
            match self.definitions.get(name).copied() {
                None => {
                    let _ = writeln!(oss, "no such definition '{}'", name);
                    self.flush();
                    self.prompt();
                    return Ok(self);
                }
                Some(it) => {
                    tmp = HfstTransducer::new_from_transducer(self.net(it));
                }
            }
        }

        tmp.output_project()?;
        tmp.extract_random_paths(&mut paths, number as i32)?;
        self.print_paths_two(&paths, oss, -1);
        self.flush();
        self.prompt();
        Ok(self)
    }

    // @brief Print astrings of upper language
    pub fn print_upper_words(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        self.print_words_level(name, number, oss, Level::UPPER_LEVEL)
    }

    // @brief Print random strings of upper language
    pub fn print_random_upper(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        let mut paths = HfstTwoLevelPaths::new();

        let mut tmp: HfstTransducer<B> = HfstTransducer::new();
        if name.is_empty() {
            let Some(temp) = self.top() else {
                return Ok(self);
            };
            tmp = HfstTransducer::new_from_transducer(self.net(temp));
        } else {
            match self.definitions.get(name).copied() {
                None => {
                    let _ = writeln!(oss, "no such definition '{}", name);
                    self.flush();
                    self.prompt();
                    return Ok(self);
                }
                Some(it) => {
                    tmp = HfstTransducer::new_from_transducer(self.net(it));
                }
            }
        }

        tmp.input_project()?;
        tmp.extract_random_paths(&mut paths, number as i32)?;
        self.print_paths_two(&paths, oss, -1);
        self.flush();
        self.prompt();
        Ok(self)
    }

    // @brief Print pair strings of language
    pub fn print_words(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        self.print_words_level(name, number, oss, Level::BOTH_LEVELS)
    }

    // @brief Print random pair strings of language
    pub fn print_random_words(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        let tmp: NetId;
        if name.is_empty() {
            let Some(t) = self.top() else {
                return Ok(self);
            };
            tmp = t;
        } else {
            match self.definitions.get(name).copied() {
                None => {
                    let _ = writeln!(oss, "no such definition '{}'", name);
                    self.flush();
                    self.prompt();
                    return Ok(self);
                }
                Some(it) => {
                    tmp = it;
                }
            }
        }

        let mut paths = HfstTwoLevelPaths::new();
        self.net(tmp)
            .extract_random_paths(&mut paths, number as i32)?;
        self.print_paths_two(&paths, oss, -1);
        self.flush();
        self.prompt();
        Ok(self)
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-paths-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-paths-fn]
    // @brief Print \a n first paths (or all, if n is negative)
    // from \a paths to \a outfile.
    pub(super) fn print_paths_two(
        &mut self,
        paths: &HfstTwoLevelPaths,
        oss: &mut dyn std::io::Write,
        mut n: i32,
    ) -> bool {
        let mut retval = false; // if anything was printed
        let _precision = self.get_precision();

        // go through at most n paths
        for it in paths.iter() {
            if n == 0 {
                break;
            }
            let path = it.second.clone();
            let mut something_printed = false; // to control printing spaces

            if self.variables["obey-flags"] == "ON" {
                let path_input = crate::hfst_symbol_defs::symbols::to_string_vector_from_pairs(
                    &path, true, /*input side*/
                );
                if !is_valid_string(&path_input) {
                    continue;
                }
            }

            retval = true; // something will be printed

            // go through the path
            for p in path.iter() {
                let print_symbol = self.get_print_symbol(&p.0);

                // see if symbol separator (space) is needed
                if self.variables["print-space"] == "ON" // print space required
                    && something_printed                  // not first symbol shown
                    && !print_symbol.is_empty()
                // something to show
                {
                    let _ = write!(oss, " ");
                }

                let _ = write!(oss, "{}", print_symbol);

                if !print_symbol.is_empty() {
                    something_printed = true;
                }

                let print_symbol = self.get_print_symbol(&p.1);

                // see if output symbol is needed
                if !print_symbol.is_empty() // something to show
                    && p.0 != p.1
                // input and output symbols differ
                {
                    let _ = write!(oss, ":{}", print_symbol);
                }
            } // path gone through

            // if needed, print the weight
            if self.variables["print-weight"] == "ON" {
                let _ = write!(oss, "\t{}", it.first);
            }

            let _ = writeln!(oss);
            n -= 1;
        } // at most n paths gone through

        self.flush();
        retval
    }

    // @brief Print \a n first paths (or all, if n is negative)
    // from \a paths to \a outfile.
    pub(super) fn print_paths_one(
        &mut self,
        paths: &HfstOneLevelPaths,
        oss: &mut dyn std::io::Write,
        mut n: i32,
    ) -> bool {
        let mut retval = false; // if anything was printed
        let _precision = self.get_precision();

        // go through at most n paths
        for it in paths.iter() {
            let path = it.second.clone();
            let mut something_printed = false; // to control printing spaces

            if (self.variables["obey-flags"] == "ON") && !is_valid_string(&path) {
                continue;
            }

            retval = true; // something will be printed

            // go through the path
            for p in path.iter() {
                let print_symbol = self.get_print_symbol(p);

                // see if symbol separator (space) is needed
                if self.variables["print-space"] == "ON" // print space required
                    && something_printed                  // not first symbol shown
                    && !print_symbol.is_empty()
                // something to show
                {
                    let _ = write!(oss, " ");
                }

                let _ = write!(oss, "{}", print_symbol);

                if !print_symbol.is_empty() {
                    something_printed = true;
                }
            } // path gone through

            // if needed, print the weight
            if self.variables["print-weight"] == "ON" {
                let _ = write!(oss, "\t{}", it.first);
            }

            let _ = writeln!(oss);
            n -= 1;
        } // at most n paths gone through

        self.flush();
        retval
    }

    // A method used by function print_longest_string_or_its_size.
    fn print_one_string_or_its_size(
        &mut self,
        oss: &mut dyn std::io::Write,
        paths: &HfstTwoLevelPaths,
        level: &str,
        print_size: bool,
    ) -> &mut Self {
        let _ = write!(oss, "{}: ", level);
        if print_size {
            let _ = writeln!(
                oss,
                "{}",
                paths
                    .iter()
                    .next()
                    .expect("caller only invokes this for a non-cyclic, non-empty level")
                    .second
                    .len() as i32
            );
        } else {
            self.print_paths_two(paths, oss, 1);
        }
        self.flush();
        self
    }

    // @brief Print the longest string of topmost transducer in the stack
    // (if print_size is false) or the size of that string (if print_size is true)
    // to \a outfile.
    fn print_longest_string_or_its_size(
        &mut self,
        oss: &mut dyn std::io::Write,
        print_size: bool,
    ) -> crate::error::Result<&mut Self> {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        // Variables needed to find out some properties about the transducer
        let mut tmp_lower = HfstTransducer::new_from_transducer(self.net(topmost));
        let mut tmp_upper = HfstTransducer::new_from_transducer(self.net(topmost));
        tmp_lower.output_project()?.remove_epsilons()?;
        tmp_upper.input_project()?.remove_epsilons()?;

        let mut paths_upper = HfstTwoLevelPaths::new();
        let mut paths_lower = HfstTwoLevelPaths::new();
        let mut upper_is_cyclic = false;
        let mut lower_is_cyclic = false;
        let mut transducer_is_empty = false;

        // Transducer is empty if neither upper..
        let obey_flags_upper = self.variables["obey-flags"] == "ON";
        match tmp_upper.extract_longest_paths(&mut paths_upper, obey_flags_upper) {
            Ok(v) => {
                transducer_is_empty = !v;
            }
            Err(e) => {
                if matches!(e.kind, crate::error::ErrorKind::TransducerIsCyclic) {
                    upper_is_cyclic = true;
                } else {
                    return Err(e);
                }
            }
        }

        // ..nor lower paths can be extracted.
        let obey_flags_lower = self.variables["obey-flags"] == "ON";
        match tmp_lower.extract_longest_paths(&mut paths_lower, obey_flags_lower) {
            Ok(v) => {
                transducer_is_empty = !v;
            }
            Err(e) => {
                if matches!(e.kind, crate::error::ErrorKind::TransducerIsCyclic) {
                    lower_is_cyclic = true;
                } else {
                    return Err(e);
                }
            }
        }

        // Print the results:
        // first, the special cases,
        if upper_is_cyclic && lower_is_cyclic {
            println!("transducer is cyclic");
        } else if transducer_is_empty {
            println!("transducer is empty");
        }
        // then the usual:
        else {
            // warn about flag diacritics
            if self.variables["show-flags"] == "OFF"
                && (tmp_upper.has_flag_diacritics() || tmp_lower.has_flag_diacritics())
            {
                self.diag_warning(
                    "longest string may have flag diacritics that are not shown but are counted in its length ('eliminate flags' removes them)",
                );
            }

            // print one longest string of the upper level, if not cyclic
            if upper_is_cyclic {
                let _ = writeln!(oss, "Upper level is cyclic.");
            } else {
                self.print_one_string_or_its_size(oss, &paths_upper, "Upper", print_size);
            }

            // print one longest string of the lower level, if not cyclic
            if lower_is_cyclic {
                let _ = writeln!(oss, "Lower level is cyclic.");
            } else {
                self.print_one_string_or_its_size(oss, &paths_lower, "Lower", print_size);
            }
        }

        self.flush();
        self.prompt();
        Ok(self)
    }

    // @brief Try to extract a maximum of \a number paths from topmost
    // transducer in the stack and print them to \a outfile. \a level
    // defines whether the input or output level is printed or both are printed.
    fn print_words_level(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
        level: Level,
    ) -> crate::error::Result<&mut Self> {
        // [spec:hfst:def:xfst-compiler.hfst.xfst.temp-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.temp-fn]
        let mut temp: HfstTransducer<B> = HfstTransducer::new();
        if name.is_empty() {
            let Some(tmp) = self.top() else {
                return Ok(self);
            };
            temp = HfstTransducer::new_from_transducer(self.net(tmp));
        } else {
            match self.definitions.get(name).copied() {
                None => {
                    let _ = writeln!(oss, "no such definition '{}'", name);
                    self.flush();
                    self.prompt();
                    return Ok(self);
                }
                Some(it) => {
                    temp = HfstTransducer::new_from_transducer(self.net(it));
                }
            }
        }

        match level {
            Level::UPPER_LEVEL => {
                temp.input_project()?;
            }
            Level::LOWER_LEVEL => {
                temp.output_project()?;
            }
            Level::BOTH_LEVELS => {}
        }

        let mut results = HfstTwoLevelPaths::new();

        let obey_flags_off = self.variables["obey-flags"] == "OFF";
        let result = if obey_flags_off {
            temp.extract_paths(&mut results, number as i32, -1)
        } else {
            temp.extract_paths_fd(&mut results, number as i32, -1, true)
        };
        if let Err(e) = result {
            if matches!(e.kind, crate::error::ErrorKind::TransducerIsCyclic) {
                let cutoff = u32::try_from(parse_size(&self.variables["print-words-cycle-cutoff"]))
                    .expect("value out of u32 range");
                self.diag_warning(&format!(
                    "transducer is cyclic, limiting the number of cycles to {}",
                    cutoff
                ));
                if obey_flags_off {
                    temp.extract_paths(&mut results, number as i32, cutoff as i32)?;
                } else {
                    temp.extract_paths_fd(&mut results, number as i32, cutoff as i32, true)?;
                }
            } else {
                return Err(e);
            }
        }

        self.print_paths_two(&results, oss, -1);

        self.prompt();
        Ok(self)
    }
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.is-valid-string-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.is-valid-string-fn]
fn is_valid_string(sv: &crate::hfst_symbol_defs::StringVector) -> bool {
    use crate::hfst_flag_diacritics::FdOperation;
    // map features to latest values
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    // and keep track of features whose values have been negatively set
    let mut negative_values: BTreeSet<String> = BTreeSet::new();

    for it in sv.iter() {
        if FdOperation::is_diacritic(it) {
            let opstr = FdOperation::get_operator(it);
            assert!(opstr.len() == 1);
            let op = opstr.as_bytes()[0] as char;
            let feat = FdOperation::get_feature(it);
            let val = FdOperation::get_value(it);

            let is_negatively_set = negative_values.contains(&feat);

            match op {
                'P' => {
                    // positive set
                    values.insert(feat.clone(), val.clone());
                }
                'N' => {
                    // negative set
                    values.insert(feat.clone(), val.clone());
                    negative_values.insert(feat.clone());
                }
                'R' => {
                    // require
                    if val.is_empty() {
                        // empty require
                        if values.get(&feat).map(|v| v.is_empty()).unwrap_or(true) {
                            return false;
                        } else {
                            // nonempty require
                            let current = values.get(&feat).cloned().unwrap_or_default();
                            if is_negatively_set || (current != val) {
                                return false;
                            }
                        }
                    }
                }
                'D' => {
                    // disallow
                    let current = values.get(&feat).cloned().unwrap_or_default();
                    if val.is_empty() {
                        // empty disallow
                        if !current.is_empty() {
                            return false;
                        }
                    } else {
                        if (!is_negatively_set) && (current == val) {
                            // nonempty disallow
                            return false;
                        }
                    }
                }
                'C' => {
                    // clear
                    values.insert(feat.clone(), String::new());
                }
                'U' => {
                    // unification
                    let current = values.get(&feat).cloned().unwrap_or_default();
                    if current.is_empty() // if the feature is unset or
                        || ((!is_negatively_set) && (current == val)) // the feature is at this value already or
                        || (is_negatively_set && (current != val))
                    // the feature is negatively set to something else
                    {
                        values.insert(feat.clone(), val.clone());
                    } else {
                        return false;
                    }
                }
                _ => {
                    error!("ERROR: line: {}", line!());
                    panic!(); // for the compiler's peace of mind
                }
            }
        }
    }
    true
}
