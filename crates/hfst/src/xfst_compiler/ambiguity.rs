//! Ambiguity tests and extractors, substrings, and minimum-edit-distance
//! lookup. The ambiguity and edit-distance searches run in foma, which works
//! on unweighted networks; anything they produce that becomes a network on
//! the stack is composed back with the original, so its weights survive.

use super::*;

#[cfg(feature = "foma")]
use crate::backend::Backend;
#[cfg(feature = "foma")]
use crate::backend_foma::FomaTransducer;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    /// The top network as a foma network.
    #[cfg(feature = "foma")]
    fn top_as_foma(&self) -> CmdResult<FomaTransducer> {
        let top = self.top()?;
        Ok(FomaTransducer::from_basic(&self.net(top).to_basic()?)?)
    }

    #[cfg(not(feature = "foma"))]
    fn top_as_foma(&self) -> CmdResult<()> {
        Err(CommandError::new(
            "this command needs the foma backend, which this build leaves out",
        ))
    }

    // [spec:hfst:sem:xfst-cmd.ambiguity]
    /// 'test functional': no input maps to two different outputs.
    pub fn test_funct(&mut self, assertion: bool) -> CmdResult {
        #[cfg(feature = "foma")]
        {
            let mut f = self.top_as_foma()?;
            let value = foma::structures::fsm_isfunctional(&f.opts, &mut f.net);
            self.report_test(value, assertion)
        }
        #[cfg(not(feature = "foma"))]
        {
            let _ = assertion;
            self.top_as_foma()
        }
    }

    // [spec:hfst:sem:xfst-cmd.ambiguity]
    /// 'test unambiguous': no input has two different paths.
    pub fn test_unambiguous(&mut self, assertion: bool) -> CmdResult {
        #[cfg(feature = "foma")]
        {
            let mut f = self.top_as_foma()?;
            let value = foma::structures::fsm_isunambiguous(&f.opts, &mut f.net);
            self.report_test(value, assertion)
        }
        #[cfg(not(feature = "foma"))]
        {
            let _ = assertion;
            self.top_as_foma()
        }
    }

    /// The inputs of the top network that have more than one path, as an
    /// automaton in this compiler's backend.
    fn ambiguous_domain(&self) -> CmdResult<HfstTransducer<B>> {
        #[cfg(feature = "foma")]
        {
            let f = self.top_as_foma()?;
            let domain = foma::structures::fsm_extract_ambiguous_domain(&f.opts, f.net);
            let domain = FomaTransducer {
                net: domain,
                opts: f.opts,
            };
            Ok(HfstTransducer::new_from_basic(&domain.to_basic()?)?)
        }
        #[cfg(not(feature = "foma"))]
        {
            self.top_as_foma()?;
            unreachable!("top_as_foma fails without foma")
        }
    }

    /// Replace the top network with `domain` composed with it.
    fn restrict_top_to(&mut self, mut domain: HfstTransducer<B>) -> CmdResult {
        let top = self.top()?;
        let cfg = self.engine_config;
        domain.compose_with_config(self.net(top), true, &cfg)?;
        domain.optimize_with_config(&cfg)?;
        *self.net_mut(top) = domain;
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    // [spec:hfst:sem:xfst-cmd.ambiguity]
    /// 'extract ambiguous': the part of the top network whose inputs have
    /// more than one path.
    pub fn extract_ambiguous(&mut self) -> CmdResult {
        let domain = self.ambiguous_domain()?;
        self.restrict_top_to(domain)
    }

    // [spec:hfst:sem:xfst-cmd.ambiguity]
    /// 'extract unambiguous': the part of the top network whose inputs have
    /// exactly one path.
    pub fn extract_unambiguous(&mut self) -> CmdResult {
        let domain = self.ambiguous_domain()?;
        let mut unambiguous = HfstTransducer::identity_pair();
        unambiguous.repeat_star()?;
        unambiguous.subtract(&domain, true)?;
        self.restrict_top_to(unambiguous)
    }

    // [spec:hfst:sem:xfst-cmd.ambiguity]
    /// 'ambiguous upper': the inputs of the top network that have more than
    /// one path.
    pub fn ambiguous_upper(&mut self) -> CmdResult {
        let mut domain = self.ambiguous_domain()?;
        let top = self.top()?;
        let cfg = self.engine_config;
        domain.optimize_with_config(&cfg)?;
        *self.net_mut(top) = domain;
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    // [spec:hfst:sem:xfst-cmd.substring]
    /// 'substring net': every substring of every path, the empty one too.
    pub fn substring_net(&mut self) -> CmdResult {
        let top = self.top()?;
        let cfg = self.engine_config;
        // Minimizing first drops the states no path runs through, which must
        // not become starts or ends of substrings.
        let mut trimmed = HfstTransducer::new_copy(self.net(top))?;
        trimmed.minimize_with_config(&cfg)?;
        let old = trimmed.to_basic()?;

        let mut sub = HfstBasicTransducer::new();
        let epsilon = Symbol::new_static(crate::hfst_symbol_defs::internal_epsilon);
        let zero: crate::hfst_tropical_transducer_transition_data::WeightType = 0.0;
        sub.set_final_weight(0, &zero);
        for s in 0..old.get_max_state() + 1 {
            let shifted = s + 1;
            sub.add_state(shifted);
            sub.set_final_weight(shifted, &zero);
            let start = crate::hfst_basic_transition::HfstBasicTransition::new_symbols(
                shifted,
                epsilon.clone(),
                epsilon.clone(),
                zero,
                sub.coder_mut(),
            );
            sub.add_transition(0, &start, true);
            for arc in old.index(s)?.iter() {
                let copied = crate::hfst_basic_transition::HfstBasicTransition::new_symbols(
                    arc.get_target_state() + 1,
                    arc.get_input_symbol(old.coder()),
                    arc.get_output_symbol(old.coder()),
                    arc.get_weight(),
                    sub.coder_mut(),
                );
                sub.add_transition(shifted, &copied, true);
            }
        }
        for symbol in old.get_alphabet().iter() {
            sub.add_symbol_to_alphabet(symbol);
        }
        let mut result = HfstTransducer::new_from_basic(&sub)?;
        result.optimize_with_config(&cfg)?;
        *self.net_mut(top) = result;
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    // [spec:hfst:sem:xfst-cmd.apply-med]
    /// 'apply med': for each line, the closest strings of the top network's
    /// upper side by edit distance, cheapest first, one 'match<TAB>cost'
    /// line each; '???' when nothing is within 'med-cutoff'.
    // [spec:hfst:sem:xfst-cmd.apply-med]
    /// 'apply med': for each line, the closest strings of the top network's
    /// upper side by edit distance, cheapest first, one 'match<TAB>cost'
    /// line each; '???' when nothing is within 'med-cutoff'.
    pub fn apply_med(&mut self, indata: &str) -> CmdResult {
        for word in indata.split('\n').filter(|w| !w.is_empty()) {
            let matches = self.med_matches(word)?;
            if matches.is_empty() {
                println!("???");
            }
            for (m, cost) in matches {
                println!("{}\t{}", m, cost);
            }
        }
        self.prompt();
        Ok(())
    }

    // [spec:hfst:sem:xfst-cmd.apply-med]
    /// The strings of the top network's upper side closest to `word` by edit
    /// distance, cheapest first, at most 'med-limit' of them and none past
    /// 'med-cutoff'.
    pub fn med_matches(&self, word: &str) -> CmdResult<Vec<(String, i32)>> {
        #[cfg(feature = "foma")]
        {
            use foma::spelling::*;
            let top = self.top()?;
            let mut upper = HfstTransducer::new_copy(self.net(top))?;
            upper.input_project()?;
            let f = FomaTransducer::from_basic(&upper.to_basic()?)?;
            let limit = parse_size(&self.variables["med-limit"]);
            let cutoff = parse_size(&self.variables["med-cutoff"]);
            let mut medh = apply_med_init(&f.net);
            apply_med_set_heap_max(&mut medh, 4194304 + 1);
            // foma reports one match per alignment, so the same string can
            // come back several times; it is limited here, by distinct
            // strings, instead.
            apply_med_set_med_limit(&mut medh, i32::MAX);
            apply_med_set_med_cutoff(&mut medh, i32::try_from(cutoff).unwrap_or(i32::MAX));
            let mut seen = BTreeSet::new();
            let mut matches = Vec::new();
            let mut found = apply_med(&mut medh, Some(word));
            while let Some(m) = found {
                if matches.len() >= limit {
                    break;
                }
                if seen.insert(m.clone()) {
                    matches.push((m, apply_med_get_cost(&medh)));
                }
                found = apply_med(&mut medh, None);
            }
            matches.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            Ok(matches)
        }
        #[cfg(not(feature = "foma"))]
        {
            let _ = word;
            self.top_as_foma()?;
            unreachable!("top_as_foma fails without foma")
        }
    }
}
