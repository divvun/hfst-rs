//! The AST-walk driver and 'compileLexical': parsing lexc source into the
//! compiler, then building the lexicon transducer from what it collected.

use super::recode::{flag_joiner_encode, joiner_encode};
use super::*;

/// Parse a weight out of an entry gloss formatted '"weight: N"'.
///
/// Mirrors 'handle_string_entry_common' in 'lexc-parser.yy': locate the
/// 'weight:' marker, then the leading numeric run (chars in '-0.123456789'),
/// and parse it. Anything else yields '0.0'.
fn weight_from_gloss(gloss: Option<&str>) -> f64 {
    let gloss = match gloss {
        Some(g) if !g.is_empty() => g,
        _ => return 0.0,
    };
    let wstart_marker = match gloss.find("weight:") {
        Some(p) => p,
        None => return 0.0,
    };
    let digits = "-0.123456789";
    let rest = &gloss[wstart_marker..];
    let off = match rest.find(|c: char| digits.contains(c)) {
        Some(p) => wstart_marker + p,
        None => return 0.0,
    };
    let after = &gloss[off..];
    let end = match after.find(|c: char| !digits.contains(c)) {
        Some(p) => off + p,
        None => gloss.len(),
    };
    gloss[off..end].parse::<f64>().unwrap_or(0.0)
}

impl<B: AlgebraBackend> LexcCompiler<B> {
    // ----- AST-walk driver (replaces parse(FILE*) / parse(filename)) -----

    /// Name shown in source-anchored diagnostics (the lexc file name). Set
    /// before `parse` so warnings point at the right file; defaults to
    /// `"<lexc>"`.
    pub fn set_source_name(&mut self, name: &str) -> &mut Self {
        self.source_name = name.to_string();
        self
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.parse-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.parse-fn]
    /// INCREMENTAL entry point: parse 'lexc_source' via 'nfst_lexc::parse' and
    /// walk the typed AST through the registration API, accumulating its
    /// lexicons/entries into 'self' (the trie 'stringsTrie_', 'regexps',
    /// 'lexiconNames_', etc.). This is the AST-walk port of the Flex/Bison
    /// 'parse(FILE*)' / 'parse(const char*)': both '.cc' overloads ran
    /// 'hlexcparse()' (accumulating into the singleton) then
    /// 'xre.remove_defined_multichar_symbols()' and set 'parseErrors_' on
    /// failure. It does NOT run 'compileLexical': call 'compile_lexical' once
    /// after every source has been parsed. Multi-file flow: call 'parse' on each
    /// source into one compiler, then 'compile_lexical' once. Returns '&mut self'
    /// to mirror the C++ 'LexcCompiler &' chaining return.
    pub fn parse(&mut self, lexc_source: &str) -> crate::error::Result<&mut Self> {
        // Retain the source so diagnostics can render the offending snippet.
        self.source = lexc_source.to_string();
        match nfst_lexc::parse(lexc_source) {
            Ok(ast) => {
                self.compile_file(&ast.value)?;
                // mirrors 'xre.remove_defined_multichar_symbols()' in parse()
                self.xre.remove_defined_multichar_symbols();
            }
            Err(e) => {
                // mirrors the 'hlexcnerrs > 0' branch setting parseErrors_,
                // but renders the parser's spanned diagnostics first — the
                // bison yyerror printed its syntax errors before bumping
                // hlexcnerrs, so a silent flag-set would lose the message.
                for d in &e.diagnostics {
                    crate::diag::emit(
                        &self.source_name,
                        &self.source,
                        d.span.range.clone(),
                        crate::diag::Severity::Error,
                        &d.message,
                    );
                }
                self.parseErrors_ = true;
            }
        }
        Ok(self)
    }

    /// PUBLIC entry point: parse a single 'lexc_source' into 'self', then run
    /// 'compileLexical'. This is the AST-walk port of the Flex/Bison driver where
    /// tools called 'parse(...)' followed by 'compileLexical()'; it is exactly
    /// that pairing for the single-source case. Returns None on parse error
    /// (the C++ 'compileLexical' null contract expressed as an Option).
    pub fn compile(&mut self, lexc_source: &str) -> Option<HfstTransducer<B>> {
        self.parse(lexc_source).ok();
        self.compile_lexical().ok().flatten()
    }

    /// Walk the typed AST, dispatching each section/entry to the matching
    /// registration method. This is the AST-walk equivalent of the bison
    /// semantic actions ('handle_multichar' / 'handle_noflag' /
    /// 'handle_definition' / 'handle_lexicon_name' / 'handle_string_entry' /
    /// 'handle_string_pair_entry' / 'handle_regexp_entry').
    pub fn compile_file(&mut self, ast: &LexcFile) -> crate::error::Result<()> {
        for mc in &ast.multichars {
            self.current_span = mc.span.range.clone();
            self.add_alphabet(&mc.value.0);
        }
        for nf in &ast.noflags {
            self.current_span = nf.span.range.clone();
            self.add_no_flag(&nf.value.0);
        }
        for def in &ast.definitions {
            self.current_span = def.span.range.clone();
            let body = nfst_xre::pretty_print(&def.value.body);
            self.add_xre_definition(&def.value.name, &body);
        }
        for lex in &ast.lexicons {
            self.current_span = lex.span.range.clone();
            // mirror the C++ titlecase-'Lexicon' warning (lexc-parser.yy
            // LEXICON_START_WRONG_CASE), emitted before the lexicon is set.
            if lex.value.case_warning {
                if self.treat_warnings_as_errors {
                    self.error_at_current_token(
                        "Keyword 'Lexicon' used instead of 'LEXICON'. [--Werror]",
                    );
                    self.parseErrors_ = true;
                } else {
                    self.warning_at_current_token("Titlecase Lexicon parsed as LEXICON");
                }
            }
            self.set_current_lexicon_name(&lex.value.name);
            for entry in &lex.value.entries {
                self.current_span = entry.span.range.clone();
                let e = &entry.value;
                let weight = weight_from_gloss(e.gloss.as_deref());
                match &e.spec {
                    EntrySpec::Empty => {
                        self.add_string_entry("", &e.continuation, weight);
                    }
                    EntrySpec::String(s) => {
                        self.add_string_entry(s, &e.continuation, weight);
                    }
                    EntrySpec::Pair { upper, lower } => {
                        self.add_string_pair_entry(upper, lower, &e.continuation, weight);
                    }
                    EntrySpec::Regex(xre) => {
                        let r = nfst_xre::pretty_print(xre);
                        self.add_xre_entry(&r, &e.continuation, weight)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl<B: AlgebraBackend> LexcCompiler<B> {
    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.compile-lexical-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.compile-lexical-fn]
    //
    // Port of 'LexcCompiler::compileLexical()' (LexcCompiler.cc ~1300-1699).
    //
    // The C++ 'std::ostream *err = get_stream(error_)' plumbing and every
    // 'flush(err)' are folded into 'tracing' diagnostics, per the
    // module docs (the non-WINDOWS code path). The 'if (debug)' AT&T dumps are
    // omitted because the file-global 'bool debug' is always 'false'. The
    // 'COLOUR_*' '#define's are inlined as literal ANSI escapes to avoid a
    // duplicate module-scope definition with the lexc-utils body (which owns
    // 'should_colourise').
    pub fn compile_lexical(&mut self) -> crate::error::Result<Option<HfstTransducer<B>>> {
        if self.parseErrors_ {
            error!("compilation aborted due to previous errors");
            return Ok(None);
        }
        let mut warnings_generated = false;
        self.print_connectedness(&mut warnings_generated);
        if warnings_generated && self.treat_warnings_as_errors {
            error!("missing or unused LEXICONs (see above) and -Werror has been enabled");
            return Ok(None);
        }

        let mut lexicons: HfstTransducer<B> = HfstTransducer::new_from_basic(&self.stringsTrie_)?;

        lexicons.optimize()?;

        // repeat star to overgenerate
        lexicons.repeat_star()?.optimize()?;

        let mut small_substitutions = HfstSymbolSubstitutions::new();
        small_substitutions.insert(
            Symbol::new_static("@0@"),
            Symbol::new_static("@_EPSILON_SYMBOL_@"),
        );
        small_substitutions.insert(
            Symbol::new_static("@@ANOTHER_EPSILON@@"),
            Symbol::new_static("@_EPSILON_SYMBOL_@"),
        );
        small_substitutions.insert(Symbol::new_static("@ZERO@"), Symbol::new_static("0"));

        lexicons.substitute_symbol_substitutions(&small_substitutions)?;
        lexicons.prune_alphabet(true)?;

        // This graph denotes (ordinary-output-symbol | valid-joiner-pair)*.
        // Construct the closure directly: feeding a 5,000-way union through
        // generic repeat-star + determinize used gigabytes to discover this
        // same small cyclic graph.
        let mut joiners_trie = HfstBasicTransducer::new();
        joiners_trie.set_final_weight(0, &0.0);
        let add_cycle_path = |graph: &mut HfstBasicTransducer, path: &StringPairVector| {
            let mut source = 0;
            for (index, pair) in path.iter().enumerate() {
                let target = if index + 1 == path.len() {
                    0
                } else {
                    graph.add_state_new()
                };
                let transition = HfstBasicTransition::new_symbols(
                    target,
                    pair.0.clone(),
                    pair.1.clone(),
                    0.0,
                    graph.coder_mut(),
                );
                graph.add_transition(source, &transition, true);
                source = target;
            }
        };

        let mut all_joiners_to_epsilon = HfstSymbolSubstitutions::new();

        if !self.with_flags {
            let start_joiner = joiner_encode(&self.initialLexiconName_);
            let start = HfstTransducer::new_tokenized(&start_joiner, &self.tokenizer)?;
            let end_string = joiner_encode("#");
            let end = HfstTransducer::new_tokenized(&end_string, &self.tokenizer)?;
            // lexicons = start.concatenate(lexicons).concatenate(end).optimize();
            let mut bracketed = start;
            bracketed
                .concatenate(&lexicons, true)?
                .concatenate(&end, true)?
                .optimize()?;
            lexicons = bracketed;

            for s in &self.lexiconNames_ {
                if self.verbose {
                    debug!("Morphotaxing... {} ", s);
                }
                let joiner_enc = joiner_encode(s);

                // joiners trie version (later compose)
                let doubled = format!("{}{}", joiner_enc, joiner_enc);
                let new_vector = self.tokenizer.tokenize(&doubled, false);
                add_cycle_path(&mut joiners_trie, &new_vector);

                all_joiners_to_epsilon.insert(
                    Symbol::from(joiner_enc),
                    Symbol::new_static("@_EPSILON_SYMBOL_@"),
                );
            }

            let root_joiner = joiner_encode(&self.initialLexiconName_);
            let hash_joiner = joiner_encode("#");

            all_joiners_to_epsilon.insert(
                Symbol::from(root_joiner),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            );
            all_joiners_to_epsilon.insert(
                Symbol::from(hash_joiner),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            );
        } else {
            let root_p = flag_joiner_encode(&self.initialLexiconName_, false);
            let root_r = flag_joiner_encode(&self.initialLexiconName_, true);

            let start_p = HfstTransducer::new_tokenized(&root_p, &self.tokenizer)?;
            let _start_r: HfstTransducer<B> =
                HfstTransducer::new_tokenized(&root_r, &self.tokenizer)?;

            let end_string_p = flag_joiner_encode("#", false);
            let end_string_r = flag_joiner_encode("#", true);

            self.tokenizer.add_multichar_symbol(&end_string_p);
            self.tokenizer.add_multichar_symbol(&end_string_r);

            let _end_p: HfstTransducer<B> =
                HfstTransducer::new_tokenized(&end_string_p, &self.tokenizer)?;
            let end_r = HfstTransducer::new_tokenized(&end_string_r, &self.tokenizer)?;

            // lexicons = startP.concatenate(lexicons).concatenate(endR).optimize();
            let mut bracketed = start_p;
            bracketed
                .concatenate(&lexicons, true)?
                .concatenate(&end_r, true)?
                .optimize()?;
            lexicons = bracketed;

            for s in &self.lexiconNames_ {
                if self.verbose {
                    debug!("Morphotaxing... {} ", s);
                }
                let flag_p_string = flag_joiner_encode(s, false);
                let flag_r_string = flag_joiner_encode(s, true);

                // joiners trie version (later compose)
                let combined = format!("{}{}", flag_p_string, flag_r_string);
                let new_vector = self.tokenizer.tokenize(&combined, false);
                add_cycle_path(&mut joiners_trie, &new_vector);
            }
        }
        // Get the right side of every pair. Keep the temporary basic graph in
        // this narrow scope: on large lexicons retaining it through the later
        // determinization needlessly keeps a complete second graph resident.
        let right_symbols = {
            let fsm = HfstBasicTransducer::from_transducer(&lexicons);
            let mut symbols: StringSet = BTreeSet::new();
            for state in fsm.states_and_transitions() {
                for tr in state {
                    let alph2 = tr.get_output_symbol(fsm.coder());

                    if !alph2.starts_with("@@ANOTHER_EPSILON@@")
                        && !alph2.starts_with("$_LEXC_JOINER.")
                        && !alph2.starts_with("$P.LEXNAME.")
                        && !alph2.starts_with("$R.LEXNAME.")
                        && !alph2.starts_with("@_")
                    {
                        symbols.insert(alph2);
                    }
                }
            }
            symbols
        };
        for alph in &right_symbols {
            self.tokenizer.add_multichar_symbol(alph);
            let new_vector = self.tokenizer.tokenize(alph, false);
            add_cycle_path(&mut joiners_trie, &new_vector);
        }

        let joiners_all: HfstTransducer<B> = HfstTransducer::new_from_basic(&joiners_trie)?;

        lexicons
            .compose_with_config(&joiners_all, true, &self.compose_cfg())?
            .optimize()?;
        drop(joiners_all);
        drop(joiners_trie);

        let mut all_substitutions = HfstSymbolSubstitutions::new();
        if self.with_flags {
            if self.verbose {
                debug!("Changing flags...");
            }
            let mut fake_flags_to_real_flags = HfstSymbolSubstitutions::new();
            // Change fake flags to real flags
            lexicons.prune_alphabet(true)?;

            let transducer_alphabet = lexicons.get_alphabet()?;
            for s in &transducer_alphabet {
                if s.starts_with('$') && s.ends_with('$') && s.len() > 2 {
                    let alph = s.replace('$', "@");
                    fake_flags_to_real_flags.insert(s.clone(), Symbol::from(alph));
                }
            }
            all_substitutions.extend(fake_flags_to_real_flags);
        } else {
            all_substitutions.extend(all_joiners_to_epsilon);
        }

        if !all_substitutions.is_empty() {
            lexicons
                .substitute_symbol_substitutions(&all_substitutions)?
                .optimize()?;
        }
        lexicons.prune_alphabet(true)?;

        // replace reg exp key with transducers
        if self.verbose {
            debug!("Inserting regular expressions...");
        }

        // substitute all reg expression into special, unharmonizible symbols
        let mut fake_regexpr_to_real = HfstSymbolSubstitutions::new();
        for key in self.regexps.keys() {
            if key.starts_with('$') {
                // TODO: do this only for strings that look like $.....$
                let alph = key.replace('$', "@");
                fake_regexpr_to_real.insert(key.clone(), Symbol::from(alph));
            }
        }
        if !fake_regexpr_to_real.is_empty() {
            lexicons
                .substitute_symbol_substitutions(&fake_regexpr_to_real)?
                .optimize()?;
            lexicons.prune_alphabet(true)?;
        }
        let mut reg_mark_to_tr: crate::hfst_basic_transducer::SubstMap = BTreeMap::new();

        for (key, tr) in self.regexps.iter() {
            let alph = if key.starts_with('$') {
                // TODO: do this only for strings that look like $.....$
                key.replace('$', "@")
            } else {
                key.to_string()
            };
            let btr = HfstBasicTransducer::from_transducer(tr);
            reg_mark_to_tr.insert(Symbol::from(alph), btr);
        }

        let mut rv_needs_optimization = !reg_mark_to_tr.is_empty();
        let mut rv = if rv_needs_optimization {
            let mut lexicons_basic = HfstBasicTransducer::from_transducer(&lexicons);
            drop(lexicons);
            lexicons_basic.substitute_subst_map(&mut reg_mark_to_tr, true)?;
            lexicons_basic.prune_alphabet(true);
            HfstTransducer::new_from_basic(&lexicons_basic)?
        } else {
            // No regex entries means the graph is already the optimized result
            // of morphotax composition and joiner substitution. Moving it
            // avoids two full graph copies and a duplicate final minimize.
            lexicons
        };
        // Preserve only first flag of consecutive P and R lexname flag series,
        // e.g. change P.LEXNAME.1 R.LEXNAME.1 P.LEXNAME.2 R.LEXNAME.2 into
        // P.LEXNAME.1
        if self.with_flags {
            let transducer_alphabet = rv.get_alphabet()?;
            let mut flag_d: StringSet = BTreeSet::new();
            for s in &transducer_alphabet {
                if s.starts_with("@P.LEXNAME") || s.starts_with("@R.LEXNAME") {
                    flag_d.insert(s.clone());
                }
            }

            // Construct a rule for consecutive flag removal:
            // [FLAG1 | FLAG2 ... FLAGN] -> 0 || [FLAG1 | FLAG2 ... FLAGN] _
            // and also an inverted rule
            let mut flag_remover_regexp = String::from("[ ");
            let mut first_flag = true;

            for it in &flag_d {
                if !first_flag {
                    flag_remover_regexp.push_str("| ");
                }
                flag_remover_regexp.push('"');
                flag_remover_regexp.push_str(it);
                flag_remover_regexp.push_str("\" ");
                first_flag = false;
            }
            flag_remover_regexp.push(']');
            let context_regexp = flag_remover_regexp.clone();
            flag_remover_regexp.push_str(" -> 0 || ");
            flag_remover_regexp.push_str(&context_regexp);
            flag_remover_regexp.push_str(" _ ");

            let mut xre_comp: XreCompiler<B> = XreCompiler::new();

            let mut flag_filter = xre_comp
                .compile(&flag_remover_regexp)
                .expect("flag-remover regexp is generated internally and always compiles");
            flag_filter.optimize()?;
            let mut inverted_flag_filter = flag_filter.clone();
            inverted_flag_filter.invert()?.optimize()?;

            // [ [FLAG1 | FLAG2 ... FLAGN] -> 0 || [FLAG1 | FLAG2 ... FLAGN] _
            // ].inv
            //                        .o.
            //                       RESULT
            //                        .o.
            // [FLAG1 | FLAG2 ... FLAGN] -> 0 || [FLAG1 | FLAG2 ... FLAGN] _
            let mut filtered_lexicons = inverted_flag_filter;
            let cfg = self.compose_cfg();
            filtered_lexicons.compose_with_config(&rv, true, &cfg)?;
            filtered_lexicons
                .compose_with_config(&flag_filter, true, &cfg)?
                .optimize()?;

            rv.assign(&filtered_lexicons)?;
            rv_needs_optimization = false;
        }

        if rv_needs_optimization {
            rv.optimize()?;
        }
        Ok(Some(rv))
    }

    // Port of 'LexcCompiler::printConnectedness(bool &warnings_generated)'
    // (LexcCompiler.cc ~1701-1765): the missing/unused-lexicon validation. The
    // C++ 'set_difference' over the sorted 'std::set's becomes 'BTreeSet'
    // differences (also sorted). The 'COLOUR_*' escapes are dropped (the
    // 'tracing' subscriber owns formatting), and 'flush(err)' is dropped.
    pub fn print_connectedness(&self, warnings_generated: &mut bool) -> &Self {
        if self.lexiconNames_ != self.continuations {
            let lex_minus_cont: Vec<&Symbol> =
                self.lexiconNames_.difference(&self.continuations).collect();
            let cont_minus_lex: Vec<&Symbol> =
                self.continuations.difference(&self.lexiconNames_).collect();
            if !cont_minus_lex.is_empty() {
                for s in &cont_minus_lex {
                    if !self.quiet && self.warn_missing_lexicons {
                        if self.treat_warnings_as_errors {
                            error!(
                                "Sublexicon is mentioned but not defined. [-Wmissing-lexicons] ({}) ",
                                s
                            );
                        } else {
                            warn!(
                                "Sublexicon is mentioned but not defined. [-Wmissing-lexicons] ({}) ",
                                s
                            );
                        }
                    }
                    *warnings_generated = true;
                }
            }
            if !lex_minus_cont.is_empty() {
                *warnings_generated = true;
                if !self.quiet && self.warn_unused_lexicons {
                    let mut line = String::new();
                    for s in &lex_minus_cont {
                        line.push_str(s.as_str());
                        line.push(' ');
                    }
                    if self.treat_warnings_as_errors {
                        error!(
                            "Sublexicons defined but not used [-Wunused-lexicons]\n{}",
                            line
                        );
                    } else {
                        warn!(
                            "Sublexicons defined but not used [-Wunused-lexicons]\n{}",
                            line
                        );
                    }
                }
            }
        }
        self
    }
}
