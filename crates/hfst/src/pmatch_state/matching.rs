//! The top-level match loop and the best-candidate state it keeps.

use super::*;

impl PmatchContainer {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn initialize_input(&mut self, input_s: &str) {
        let boundary = self.core.alphabet.get_special(SpecialSymbol::boundary);
        let single = self.single_codepoint_tokenization;
        // The tokenizer writes the symbols it produces straight into 'input',
        // which it cannot borrow while admitting into 'self'; hand it the vector
        // and put it back.
        let mut input = std::mem::take(&mut self.input);
        tokenize_input(self, boundary, single, input_s, &mut input);
        self.input = input;
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.has-unsatisfied-rtns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.has-unsatisfied-rtns-fn]
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-unsatisfied-rtn-name-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-unsatisfied-rtn-name-fn]
    // Never true, so there is never an unsatisfied RTN name to report either.
    pub fn has_unsatisfied_rtns(&self) -> bool {
        false
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.process-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.process-fn]
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn process(&mut self, input: &str) {
        if self.verbose {
            debug!("PC::processing {}", input);
        }
        self.initialize_input(input);
        let mut input_pos: u32 = 0;
        let mut printable_input_pos: u32 = 0;
        self.running_weight = 0.0;
        self.stack_depth = 0;
        self.best_input_pos = 0;

        self.line_number += 1;
        self.result.inner.clear();
        self.locations.clear();
        self.old_captures.clear();
        self.best_captures.clear();
        self.captures.clear();
        self.reset_recursion();
        // A handle of our own on the archive, so the walk can borrow nets out of
        // it while the run state below is borrowed exclusively.
        let core = Arc::clone(&self.core);
        let toplevel = core
            .toplevel
            .as_deref()
            .expect("toplevel present for match");
        let mut nonmatching_locations = DoubleTape::new();
        while self.has_queued_input(input_pos) {
            self.best_result.inner.clear();
            let current_input = self.input[input_pos as usize];
            if self.core.not_possible_first_symbol(current_input) {
                self.copy_to_result_syms(current_input, current_input);
                input_pos += 1;
                if self.props.locate_mode && self.is_printable_sym(current_input) {
                    printable_input_pos += 1;
                    nonmatching_locations
                        .inner
                        .push(crate::transducer::SymbolPair::new_values(
                            current_input,
                            current_input,
                        ));
                }
                continue;
            }
            self.tape.inner.clear();
            self.tape_locations.clear();
            let tape_pos: u32 = 0;
            let old_input_pos = input_pos;
            // toplevel->match(input_pos, tape_pos);
            PmatchWalk::entering(&core, toplevel).do_match(input_pos, tape_pos, self);
            if self.candidate_found() {
                // We got some output
                if self.props.locate_mode {
                    // First we put into the locations vector all the
                    // nonmatching parts we've seen
                    if !nonmatching_locations.inner.is_empty() {
                        let mut ls: LocationVector = LocationVector::new();
                        let mut nonmatching = self.locatefy(
                            printable_input_pos
                                - u32::try_from(nonmatching_locations.inner.len())
                                    .expect("value out of u32 range"),
                            &WeightedDoubleTape::new(nonmatching_locations.clone(), 0.0),
                        );
                        nonmatching.output = "@_NONMATCHING_@".to_string();
                        if self.verbose {
                            debug!("non-matching {}", nonmatching.input);
                        }
                        ls.push(nonmatching);
                        self.locations.push(ls);
                        nonmatching_locations.inner.clear();
                    }
                    let mut ls: LocationVector = LocationVector::new();
                    let tape_locations = self.tape_locations.clone();
                    for it in tape_locations.iter() {
                        let l = self.locatefy(printable_input_pos, it);
                        if self.verbose {
                            debug!("located? {}:{}", l.input, l.output);
                        }
                        ls.push(l);
                    }
                    ls.sort();
                    // The walk can reach one accepting configuration through
                    // several structurally distinct paths (e.g. a union branch
                    // carrying an extra EndTag), and 'locatefy' projects away
                    // every non-printable, non-endtag symbol, so those paths
                    // collapse to byte-identical Locations. Each is reported
                    // separately, exactly as C++ does: the multiplicity of a
                    // reading inside a cohort is part of the Constraint Grammar
                    // contract that consumers of '-c'/'-g' depend on, so the
                    // vector is passed through unfiltered. (Upstream offers an
                    // opt-in '-u' flag for callers who want uniqueness; it is
                    // not applied here by default.)
                    self.locations.push(ls);
                    printable_input_pos += self.best_input_pos - old_input_pos;
                } else {
                    let best_result = self.best_result.clone();
                    self.copy_to_result(&best_result);
                }
                input_pos = self.best_input_pos;
                let best_captures = std::mem::take(&mut self.best_captures);
                self.old_captures.extend(best_captures.iter().cloned());
                self.best_captures = best_captures;
            }
            if !self.candidate_found() || input_pos == old_input_pos {
                // If no input was consumed, we move one position up
                if self.verbose {
                    debug!("no candidate found");
                }
                self.copy_to_result_syms(current_input, current_input);
                input_pos += 1;
                if self.props.locate_mode && self.is_printable_sym(current_input) {
                    printable_input_pos += 1;
                    nonmatching_locations
                        .inner
                        .push(crate::transducer::SymbolPair::new_values(
                            current_input,
                            current_input,
                        ));
                }
            }
        }
        if self.props.locate_mode && !nonmatching_locations.inner.is_empty() {
            let mut ls: LocationVector = LocationVector::new();
            let mut nonmatching = self.locatefy(
                printable_input_pos
                    - u32::try_from(nonmatching_locations.inner.len())
                        .expect("value out of u32 range"),
                &WeightedDoubleTape::new(nonmatching_locations.clone(), 0.0),
            );
            nonmatching.output = "@_NONMATCHING_@".to_string();
            if self.verbose {
                debug!("nonmatching somethign or other{}", nonmatching.input);
            }
            ls.push(nonmatching);
            self.locations.push(ls);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.match-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.match-fn]
    pub fn do_match(&mut self, input: &str, time_cutoff: f64, weight_cutoff: Weight) -> String {
        self.max_time = time_cutoff;
        self.max_weight = weight_cutoff;
        if self.max_time > 0.0 {
            self.start_clock = Some(Instant::now());
            self.call_counter = 0;
            self.limit_reached = false;
        }
        self.props.locate_mode = false;
        self.process(input);
        let result = self.result.clone();
        self.stringify(&result)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.locate-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.locate-fn]
    pub fn locate(
        &mut self,
        input: &str,
        time_cutoff: f64,
        weight_cutoff: Weight,
    ) -> LocationVectorVector {
        if self.verbose {
            debug!("locating {}", input);
        }
        self.max_time = time_cutoff;
        self.max_weight = weight_cutoff;
        if self.max_time > 0.0 {
            self.start_clock = Some(Instant::now());
            self.call_counter = 0;
            self.limit_reached = false;
        }
        self.props.locate_mode = true;
        self.process(input);
        self.locations.clone()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.note-analysis-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.note-analysis-fn]
    pub fn note_analysis(&mut self, input_pos: u32, tape_pos: u32) {
        if (input_pos > self.best_input_pos)
            || (input_pos == self.best_input_pos && self.best_weight > self.running_weight)
        {
            self.best_result = self.tape.extract_slice(0, tape_pos);
            self.best_captures = self.captures.clone();
            self.best_input_pos = input_pos;
            self.best_weight = self.running_weight;
        } else if self.verbose
            && input_pos == self.best_input_pos
            && self.best_weight == self.running_weight
        {
            let discarded = self.tape.extract_slice(0, tape_pos);
            let best_result = self.best_result.clone();
            let kept = self.stringify(&best_result);
            let disc = self.stringify(&discarded);
            debug!(
                "\n\tline {}: conflicting equally weighted matches found, keeping:\n\t{}\n\tdiscarding:\n\t{}\n",
                self.line_number, kept, disc
            );
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.grab-location-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.grab-location-fn]
    pub fn grab_location(&mut self, input_pos: u32, tape_pos: u32) {
        if !self.tape_locations.is_empty() {
            if input_pos < self.best_input_pos {
                // We already have better matches
                return;
            } else if input_pos > self.best_input_pos {
                // The old locations are worse
                self.best_captures.clear();
                self.tape_locations.clear();
            }
        }
        self.best_input_pos = input_pos;
        self.best_captures = self.captures.clone();
        let rv = WeightedDoubleTape::new(self.tape.extract_slice(0, tape_pos), self.running_weight);
        self.tape_locations.push(rv);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-longest-matching-capture-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-longest-matching-capture-fn]
    // C++ returns a pair of iterators into 'input'; we return the (begin, end)
    // indices into 'self.input' instead (an empty match is begin == end).
    pub fn get_longest_matching_capture(
        &mut self,
        key: SymbolNumber,
        input_pos: u32,
    ) -> (usize, usize) {
        // longest_so_far(input.begin(), input.begin())
        let mut longest_so_far: (usize, usize) = (0, 0);
        let captures = self.captures.clone();
        for it in captures.iter() {
            if key == it.name
                && self.input_matches_at(input_pos, it.begin as usize, it.end as usize)
                && (it.end - it.begin) as usize > longest_so_far.1 - longest_so_far.0
            {
                longest_so_far.0 = it.begin as usize;
                longest_so_far.1 = it.end as usize;
            }
        }
        let old_captures = self.old_captures.clone();
        for it in old_captures.iter() {
            if key == it.name
                && self.input_matches_at(input_pos, it.begin as usize, it.end as usize)
                && (it.end - it.begin) as usize > longest_so_far.1 - longest_so_far.0
            {
                longest_so_far.0 = it.begin as usize;
                longest_so_far.1 = it.end as usize;
            }
        }
        longest_so_far
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.has-queued-input-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.has-queued-input-fn]
    pub fn has_queued_input(&self, input_pos: u32) -> bool {
        // we catch underflow due to left context checking here
        (input_pos as usize) < self.input.len() && (input_pos.wrapping_add(1) != 0)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.input-matches-at-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.input-matches-at-fn]
    // begin/end are indices into self.input (matching get_longest_matching_capture).
    pub fn input_matches_at(&self, pos: u32, begin: usize, end: usize) -> bool {
        // if (pos + (end - begin) >= input.size()) return false;
        if pos as usize + (end - begin) >= self.input.len() {
            return false;
        }
        let mut i: usize = 0;
        while begin + i != end {
            if self.input[pos as usize + i] != self.input[begin + i] {
                return false;
            }
            i += 1;
        }
        true
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.copy-to-result-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.copy-to-result-fn]
    pub fn copy_to_result(&mut self, best_result: &DoubleTape) {
        for it in best_result.inner.iter() {
            self.result.inner.push(*it);
        }
    }
    pub fn copy_to_result_syms(&mut self, input: SymbolNumber, output: SymbolNumber) {
        self.result
            .inner
            .push(crate::transducer::SymbolPair::new_values(input, output));
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.candidate-found-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.candidate-found-fn]
    pub fn candidate_found(&self) -> bool {
        if self.props.locate_mode {
            !self.tape_locations.is_empty()
        } else {
            !self.best_result.inner.is_empty()
        }
    }
}
