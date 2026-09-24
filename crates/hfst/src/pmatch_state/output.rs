//! Rendering a run's tapes as text and locations, and its reports.

use super::*;

impl PmatchContainer {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.stringify-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.stringify-fn]
    // The C++ calls 'alphabet.stringify(.., this)': the alphabet reads its own
    // symbol tables while writing the container's pattern counts. Here the tape
    // is a run's, and so are the counts and the overlay that names any symbol
    // this run admitted, so the whole rendering belongs to the run state.
    pub(super) fn stringify(&mut self, str: &DoubleTape) -> String {
        let mut retval = String::new();
        let mut start_tag_pos: Vec<u32> = Vec::new();
        let mut input_contained_printable_symbol = false;
        for it in str.inner.clone().iter() {
            if !input_contained_printable_symbol && self.is_printable_sym(it.input) {
                input_contained_printable_symbol = true;
            }
            let output = it.output;
            if output == self.core.alphabet.get_special(SpecialSymbol::entry) {
                start_tag_pos.push(u32::try_from(retval.len()).expect("value out of u32 range"));
            } else if output == self.core.alphabet.get_special(SpecialSymbol::exit) {
                if !start_tag_pos.is_empty() {
                    start_tag_pos.pop();
                }
            } else if self.core.alphabet.is_end_tag_sym(output) {
                if self.props.count_patterns && input_contained_printable_symbol {
                    let key = self.core.alphabet.start_tag(output);
                    *self.pattern_counts.entry(key).or_insert(0) += 1;
                }
                let pos: u32 = if start_tag_pos.is_empty() {
                    warn!("end tag without start tag");
                    0
                } else {
                    *start_tag_pos
                        .last()
                        .expect("stack non-empty in else branch")
                };
                if self.props.delete_patterns {
                    let how_much_to_delete = retval.len() - pos as usize;
                    retval.replace_range(
                        pos as usize..pos as usize + how_much_to_delete,
                        &self.core.alphabet.start_tag(output),
                    );
                } else if self.props.mark_patterns && input_contained_printable_symbol {
                    retval.insert_str(pos as usize, &self.core.alphabet.start_tag(output));
                    retval.push_str(&self.core.alphabet.end_tag(output));
                }
            } else if (!self.props.extract_patterns || !start_tag_pos.is_empty())
                && self.is_printable_sym(output)
            {
                retval.push_str(&self.symbol_string(output));
            }
        }
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.locatefy-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.locatefy-fn]
    pub(super) fn locatefy(&mut self, input_offset: u32, str: &WeightedDoubleTape) -> Location {
        let mut retval = Location {
            start: input_offset,
            weight: str.weight,
            ..Default::default()
        };
        let mut input_offset = input_offset;
        let mut input_mark: usize = 0;
        let mut output_mark: usize = 0;

        // We rebuild the original input without special
        // symbols but with IDENTITIES etc. replaced
        for it in str.tape.inner.iter() {
            let input = it.input;
            let output = it.output;
            if self.core.alphabet.is_end_tag_sym(output) {
                if self.props.count_patterns {
                    let key = self.core.alphabet.start_tag(output);
                    *self.pattern_counts.entry(key).or_insert(0) += 1;
                }
                retval.tag = self.core.alphabet.start_tag(output);
                continue;
            }
            if self.is_printable_sym(output) {
                let s = self.symbol_string(output);
                retval.output.push_str(&s);
                retval.output_symbol_strings.push(s);
            }
            if self.is_printable_sym(input) {
                let s = self.symbol_string(input);
                retval.input.push_str(&s);
                retval.input_symbol_strings.push(s);
                input_offset += 1;
            }
            if self.core.alphabet.is_input_mark(output) {
                retval.output_parts.push(output_mark);
                retval.input_parts.push(input_mark);
                output_mark = retval.output_symbol_strings.len();
                input_mark = retval.input_symbol_strings.len();
            }
        }
        if output_mark > 0 {
            retval.output_parts.push(output_mark);
        }
        if input_mark > 0 {
            retval.input_parts.push(input_mark);
        }
        retval.length = input_offset - retval.start;
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-profiling-info-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-profiling-info-fn]
    pub fn get_profiling_info(&mut self) -> String {
        let mut retval = String::new();
        let mut max_name_len: usize = 0;
        retval.push_str("Profiling information:\n");
        retval.push_str("  Traversals of Counter() positions:\n");
        let mut counter_name_val_pairs: Vec<(String, u64)> = Vec::new();
        let counters = self.counters().to_vec();
        for (i, tally) in counters.iter().enumerate() {
            if *tally != NO_COUNTER {
                let counter_name = self.core.alphabet.get_counter_name(i as SymbolNumber);
                if counter_name.len() > max_name_len {
                    max_name_len = counter_name.len();
                }
                counter_name_val_pairs.push((counter_name, *tally));
            }
        }
        // std::sort with counter_comp (descending by .1)
        counter_name_val_pairs.sort_by(|a, b| {
            if counter_comp(a.clone(), b.clone()) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });
        for it in counter_name_val_pairs.iter() {
            retval.push_str("    ");
            retval.push_str(&it.0);
            let mut spacing_counter = max_name_len + 8 - it.0.len();
            while spacing_counter != 0 {
                retval.push(' ');
                spacing_counter -= 1;
            }
            retval.push_str(&it.1.to_string());
            retval.push('\n');
        }
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-pattern-count-info-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-pattern-count-info-fn]
    pub fn get_pattern_count_info(&mut self) -> String {
        let mut total: usize = 0;
        let mut retval = String::from("Pattern\t\t# of matches\n------------------------\n");
        for (first, second) in self.pattern_counts.iter() {
            retval.push_str(first);
            retval.push_str("\t\t");
            retval.push_str(&second.to_string());
            retval.push('\n');
            total += *second;
        }
        retval.push_str("------------------------\n");
        retval.push_str("Total:\t\t");
        retval.push_str(&total.to_string());
        retval.push('\n');
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.uncompose-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.uncompose-fn]
    pub fn uncompose(&mut self, loc: &mut Location) {
        let verbose = self.verbose;
        if !self.props.uncomposable {
            if verbose {
                debug!("uncompose disabled");
            }
            return;
        }
        if verbose {
            debug!("uncomposing left {}", loc.input);
        }
        let middle_left = self
            .core
            .uncompose_left
            .as_ref()
            .expect("uncompose_left set when uncomposable")
            .lookup_fd_str(&loc.input, -1, 0.0);
        if middle_left.is_empty() {
            if verbose {
                debug!("empty midleft compose");
            }
            // ambig problems
            return;
        }
        let mut midforms: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for lpath in &middle_left {
            let mut mids = String::new();
            for symbol in &lpath.second {
                if !crate::hfst_flag_diacritics::FdOperation::is_diacritic(symbol) {
                    mids.push_str(symbol);
                }
            }
            if verbose {
                debug!("midleft composed {}", mids);
            }
            let middle_right = self
                .core
                .uncompose_right
                .as_ref()
                .expect("uncompose_right set when uncomposable")
                .lookup_fd_str(&mids, -1, 0.0);
            if middle_right.is_empty() {
                if verbose {
                    debug!("empty midright compose");
                }
                continue;
            }
            for rpath in &middle_right {
                let mut lows = String::new();
                for rsym in &rpath.second {
                    if !crate::hfst_flag_diacritics::FdOperation::is_diacritic(rsym) {
                        lows.push_str(rsym);
                    }
                }
                if verbose {
                    debug!("midright composed {}", lows);
                }
                if lows == loc.output {
                    if verbose {
                        debug!("matched {}", loc.output);
                    }
                    midforms.insert(mids.clone());
                } else if verbose {
                    debug!("no match {}", loc.output);
                }
            }
        }
        if midforms.len() > 1 {
            // ambig problems
        }
        for form in &midforms {
            loc.middle = form.clone();
        }
    }
}
