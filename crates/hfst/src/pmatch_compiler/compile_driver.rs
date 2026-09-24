//! The compile entry points, include expansion, file reading, and archive writing.

use super::*;

// ---------------------------------------------------------------------------
// Bison-bridge globals/functions consumed by this group (the bison parser is
// replaced by the nfst-pmatch walk, but 'compile'/'init_globals'/
// 'expand_includes' still reference these). The integrator links them if
// another group provides them; otherwise these definitions stand in.
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.pmatcherror-fn]
// [spec:hfst:sem:pmatch-utils.pmatcherror-fn]
// [spec:hfst:def:pmatch-utils.pmatchwarning-fn]
// [spec:hfst:sem:pmatch-utils.pmatchwarning-fn]
pub fn pmatchwarning(msg: &str) {
    warn!("pmatch: {}", msg);
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.expand-includes-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.expand-includes-fn]
pub fn expand_includes<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    script: &str,
) -> String {
    if !script.contains("@include\"") {
        return script.to_string();
    }
    let mut in_quoted_literal = false;
    let mut in_curly_literal = false;
    let mut in_comment = false;
    let bytes = script.as_bytes();
    let mut idx: usize = 0;
    let mut retval = String::new();
    while idx < bytes.len() {
        let c = bytes[idx] as char;
        if in_quoted_literal && c == '"' && (idx == 0 || bytes[idx - 1] != b'\\') {
            in_quoted_literal = false;
        } else if in_curly_literal && c == '}' && (idx == 0 || bytes[idx - 1] != b'\\') {
            in_curly_literal = false;
        } else if in_comment && c == '\n' {
            in_comment = false;
        } else if c == '"' {
            in_quoted_literal = true;
        } else if c == '{' {
            in_curly_literal = true;
        } else if c == '!' {
            in_comment = true;
        } else if c == '%' {
            retval.push(bytes[idx] as char);
            idx += 1;
            if idx < bytes.len() {
                retval.push(bytes[idx] as char);
                idx += 1;
            }
            continue;
        } else if bytes[idx..].starts_with(b"@include\"") {
            let terminating_quote_pos = script[idx + 9..].find('"').map(|p| p + idx + 9);
            if let Some(terminating_quote_pos) = terminating_quote_pos {
                let filename_start_pos = idx + 9;
                let filename_len = terminating_quote_pos - filename_start_pos;
                let filepath = path_from_filename(
                    ctx,
                    &script[filename_start_pos..filename_start_pos + filename_len],
                );
                match fs::read(&filepath) {
                    Ok(contents) => {
                        for b in contents {
                            retval.push(b as char);
                        }
                    }
                    Err(_) => {
                        let errstring = format!("could not open file {} for @include\n", filepath);
                        ctx.pmatcherror(&errstring);
                    }
                }
                idx += 10 + filename_len;
                continue;
            }
        }
        retval.push(bytes[idx] as char);
        idx += 1;
    }
    retval
}
// Render a parser failure as source-anchored diagnostics. The bison port used
// to swallow 'nfst_pmatch::parse' errors and let compilation fall through to
// the downstream "Empty ruleset, nothing to write" message, which hides the
// real cause — e.g. a reserved predefined acceptor name (`Alpha`,
// `Whitespace`, ...) used as a `Define` target. Each diagnostic carries a byte
// span into the include-expanded script, so the rendered snippet is anchored
// there.
fn emit_pmatch_parse_error(e: &nfst_pmatch::ParseError, expanded_script: &str) {
    if e.diagnostics.is_empty() {
        error!("pmatch: syntax error");
    }
    for d in &e.diagnostics {
        crate::diag::emit(
            "<pmatch>",
            expanded_script,
            d.span.range.clone(),
            crate::diag::Severity::Error,
            &format!("pmatch: syntax error: {}", d.message),
        );
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-fn]
pub fn compile<B: AlgebraBackend + FromAnyTransducer + 'static>(
    pmatch: &str,
    defs: &HashMap<String, HfstTransducer<B>>,
    be_verbose: bool,
    do_flatten: bool,
    do_include_cosine_distances: bool,
    includedir: String,
) -> crate::error::Result<HashMap<String, HfstTransducer<B>>> {
    // lock here?
    let mut ctx_owned = PmatchEvalContext::new();
    let ctx = &mut ctx_owned;
    ctx.init_globals();
    let expanded_script = expand_includes(ctx, pmatch);
    ctx.data = expanded_script.clone();
    ctx.len = ctx.data.clone().len();
    ctx.verbose = be_verbose;
    ctx.flatten = do_flatten;
    ctx.include_cosine_distances = do_include_cosine_distances;
    ctx.includedir = includedir;
    ctx.vector_similarity_projection_factor = 1.0;
    for (key, value) in defs.iter() {
        ctx.definitions_insert(
            key.clone(),
            as_obj(PmatchTransducerContainer::new(HfstTransducer::new_copy(
                value,
            )?)),
        );
    }
    if ctx.verbose {
        ctx.timer = clock();
        debug!("");
    }

    // === SEAM: replaces the bison 'pmatchparse()' call ====================
    // The build-driver group walks the nfst-pmatch parse tree, populating the
    // 'hfst::pmatch' globals exactly as the bison actions would have.
    match nfst_pmatch::parse(&expanded_script) {
        Ok(parsed) => {
            for statement in &parsed.value.statements {
                if let Err(e) = build_statement(ctx, statement) {
                    error!("{}", e);
                    ctx.pmatchnerrs += 1;
                }
            }
        }
        Err(e) => {
            emit_pmatch_parse_error(&e, &expanded_script);
            ctx.pmatchnerrs += 1;
        }
    }
    // === END SEAM =========================================================

    let mut retval: HashMap<String, HfstTransducer<B>> = HashMap::new();
    for it in ctx.unsatisfied_insertions_snapshot().into_iter() {
        if !ctx.definitions_contains(it.as_str()) {
            error!("Inserted transducer {} was never defined!", it);
            return Ok(retval);
        }
    }
    if ctx.verbose {
        let defs_keys: Vec<String> = ctx.definitions_keys();
        for first in defs_keys.iter() {
            if !ctx.used_definitions_contains(first) && first != "TOP" {
                debug!("Warning: {} defined but never used", first);
            }
        }
    }

    if ctx.pmatchnerrs != 0 {
        ctx.data = String::new();
        ctx.len = 0;
        return Ok(retval);
    }
    // Our helper for harmonizing all the networks' alphabets with
    // each other
    if ctx.verbose {
        debug!("Compiling and harmonizing...");
        ctx.timer = clock();
    }

    let mut uncount: u32 = 0;
    if ctx.inserted_names_len() > 0
        || ctx.def_insed_expressions_len() > 0
        || ctx.uncomposed_len() > 0
    {
        let mut dummy = HfstTransducer::new();
        // We keep TOP and any inserted transducers
        let defs_keys: Vec<String> = ctx.definitions_keys();
        for first in defs_keys.iter() {
            if first == "TOP"
                || ctx.inserted_names_contains(first)
                || ctx.def_insed_expressions_contains(first)
                || ctx.uncomposed_contains(first)
            {
                if ctx.verbose {
                    let second = ctx
                        .definitions_get(first)
                        .expect("first is a definitions key");
                    debug!("definition...{}={}", first, second.get_name().to_string());
                }
                let mut tmp: HfstTransducer<B> = if ctx.def_insed_expressions_contains(first) {
                    ctx.def_insed_expressions_get(first)
                        .expect("checked with contains just above")
                        .evaluate(ctx)?
                } else {
                    ctx.definitions_get(first)
                        .expect("first is a definitions key")
                        .evaluate(ctx)?
                };
                tmp.minimize()?;
                dummy.harmonize(&mut tmp, true)?;
                // This is what it will be called in the archive
                // XXX: seems to use the index not the name...)
                if ctx.uncomposed_contains(first) {
                    if ctx.verbose {
                        debug!("Uncompose");
                    }
                    if uncount == 0 {
                        tmp.set_name(&("UNCOMPOSE LEFT ".to_string() + first));
                        retval.insert("UNCOMPOSE LEFT ".to_string() + first, tmp);
                        uncount += 1;
                    } else if uncount == 1 {
                        tmp.set_name(&("UNCOMPOSE RIGHT ".to_string() + first));
                        retval.insert("UNCOMPOSE RIGHT ".to_string() + first, tmp);
                        uncount += 1;
                    } else {
                        warn!("Uncompose only works once so far...");
                        uncount += 1;
                    }
                } else {
                    tmp.set_name(first);
                    retval.insert(first.clone(), tmp);
                }
            }
        }

        // Now that dummy is harmonized with everything, we harmonize
        // everything with dummy and minimize the results
        for second in retval.values_mut() {
            second.harmonize(&mut dummy, true)?;
            second.minimize()?;
        }
    } else {
        if ctx.definitions_len() == 0 {
            warn!("pmatch compilation had an empty result");
            retval.insert("TOP".to_string(), HfstTransducer::new());
        } else if !ctx.definitions_contains("TOP") {
            let first_key = ctx
                .definitions_keys()
                .into_iter()
                .next()
                .expect("definitions non-empty in this branch");
            warn!(
                "Pmatch compilation: regex or TOP was undefined, using {} as root",
                first_key
            );
            let mut tmp = ctx
                .definitions_get(&first_key)
                .expect("first_key is a definitions key")
                .evaluate(ctx)?;
            tmp.minimize()?;
            tmp.set_name("TOP");
            retval.insert("TOP".to_string(), tmp);
        } else {
            let mut tmp = ctx
                .definitions_get("TOP")
                .expect("TOP present in this branch")
                .evaluate(ctx)?;
            tmp.minimize()?;
            tmp.set_name("TOP");
            retval.insert("TOP".to_string(), tmp);
        }
    }

    if ctx.verbose {
        let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
        ctx.timer = clock();
        debug!("Everything compiled and harmonized in {} seconds", duration);
    }

    set_initial_symbol_variables(ctx)?;
    if ctx.variables_get("need-separators").as_deref() == Some("on") {
        let whitespace_acc =
            ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_whitespace_acceptor))?;
        let punct_acc = ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_punct_acceptor))?;
        let mut not_whitespace = HfstTransducer::new_symbol(internal_identity)?;
        not_whitespace.subtract(&whitespace_acc, true)?;
        let mut anything = HfstTransducer::new_symbol(internal_identity)?;
        anything.repeat_star()?;
        let mut begins_and_ends_with_non_whitespace = HfstTransducer::new_copy(&not_whitespace)?;
        begins_and_ends_with_non_whitespace.concatenate(&anything, true)?;
        begins_and_ends_with_non_whitespace.concatenate(&not_whitespace, true)?;
        begins_and_ends_with_non_whitespace
            .compose(retval.get("TOP").expect("TOP defined above"), true)?;
        let mut is_single_non_whitespace = HfstTransducer::new_copy(&not_whitespace)?;
        is_single_non_whitespace.compose(retval.get("TOP").expect("TOP defined above"), true)?;
        let empty = HfstTransducer::new();
        if !begins_and_ends_with_non_whitespace.compare(&empty, true)?
            || !is_single_non_whitespace.compare(&empty, true)?
        {
            let mut whitespace_punct_context = HfstTransducer::new_copy(&whitespace_acc)?;
            whitespace_punct_context.disjunct(&punct_acc, true)?;
            whitespace_punct_context.disjunct(&HfstTransducer::new_symbol("@BOUNDARY@")?, true)?;
            let mut top_with_boundaries: HfstTransducer<B> =
                HfstTransducer::new_symbol_pair(internal_epsilon, LC_ENTRY_SYMBOL)?;
            top_with_boundaries.concatenate(&whitespace_punct_context, true)?;
            top_with_boundaries.concatenate(
                &HfstTransducer::new_symbol_pair(internal_epsilon, LC_EXIT_SYMBOL)?,
                true,
            )?;
            let mut rc = HfstTransducer::new_symbol_pair(internal_epsilon, RC_ENTRY_SYMBOL)?;
            rc.concatenate(&whitespace_punct_context, true)?;
            rc.concatenate(
                &HfstTransducer::new_symbol_pair(internal_epsilon, RC_EXIT_SYMBOL)?,
                true,
            )?;
            top_with_boundaries.concatenate(retval.get("TOP").expect("TOP defined above"), true)?;
            top_with_boundaries.concatenate(&rc, true)?;
            retval.insert(
                "TOP".to_string(),
                add_pmatch_delimiters(&top_with_boundaries)?,
            );
            retval
                .get_mut("TOP")
                .expect("TOP defined above")
                .minimize()?;
            if ctx.verbose {
                let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
                ctx.timer = clock();
                debug!("Added automatic context separators in {} seconds", duration);
            }
        }
    }
    let vars: Vec<(String, String)> = ctx.variables_snapshot();
    let top = retval
        .get_mut("TOP")
        .expect("TOP present in result by this point");
    for (key, value) in vars.iter() {
        top.set_property(key, value);
    }
    ctx.data = String::new();
    ctx.len = 0;
    Ok(retval)
}
// Collect TOP's allowed and disallowed initial symbols and store them as the
// 'initial-symbols' and 'disallowed-initial-symbols' variables, unless
// something in the lists looks suspicious.
fn set_initial_symbol_variables<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<()> {
    let mut allowed_initial_symbols: StringSet = StringSet::new();
    let mut disallowed_initial_symbols: StringSet = StringSet::new();
    ctx.definitions_get("TOP")
        .expect("TOP defined by this point")
        .collect_initial_symbols_into(
            &mut allowed_initial_symbols,
            &mut disallowed_initial_symbols,
        )?;
    let mut initial_symbols_list = String::new();
    let mut disallowed_initial_symbols_list = String::new();
    // Use this to bail out if there's something suspicious in the final lists
    let mut initial_symbols_ok = true;
    for it in allowed_initial_symbols.iter() {
        if is_special(it) {
            if ctx.verbose {
                debug!(
                    "Not setting initial symbol list due to special symbol {}",
                    it
                );
            }
            initial_symbols_ok = false;
        }
        initial_symbols_list.push_str(it);
    }
    for it in disallowed_initial_symbols.iter() {
        if is_special(it) {
            if ctx.verbose {
                debug!(
                    "Not setting initial symbol list due to special symbol {}",
                    it
                );
            }
            initial_symbols_ok = false;
        }
        disallowed_initial_symbols_list.push_str(it);
    }
    if allowed_initial_symbols.len() > 200 {
        if ctx.verbose {
            debug!(
                "Not setting initial symbol list due to excess length: {}",
                allowed_initial_symbols.len()
            );
        }
        initial_symbols_ok = false;
    }
    if disallowed_initial_symbols.len() > 200 {
        if ctx.verbose {
            debug!(
                "Not setting initial symbol list due to excess length: {}",
                disallowed_initial_symbols.len()
            );
        }
        initial_symbols_ok = false;
    }
    if initial_symbols_ok && !initial_symbols_list.is_empty() {
        ctx.variables_insert("initial-symbols".to_string(), initial_symbols_list);
    }
    if initial_symbols_ok && !disallowed_initial_symbols_list.is_empty() {
        ctx.variables_insert(
            "disallowed-initial-symbols".to_string(),
            disallowed_initial_symbols_list,
        );
    }
    Ok(())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.write-compilation-stack-indentation-to-err-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.write-compilation-stack-indentation-to-err-fn]
pub fn write_compilation_stack_indentation_to_err<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) {
    // Visually indicate nested definitions
    let mut indentation = String::new();
    let mut i = 1;
    while i < ctx.named_object_evaluation_stack_depth {
        indentation.push('|');
        i += 1;
    }
    if ctx.named_object_evaluation_stack_depth > 1 {
        indentation.push(' ');
    }
    if !indentation.is_empty() {
        debug!("{}", indentation);
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-text-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-text-fn]
pub fn read_text<B: AlgebraBackend>(
    filename: String,
    spaced_text: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let tok = HfstTokenizer::new();
    let mut retval: HfstTransducer<B> = HfstTransducer::new();
    match fs::read_to_string(&filename) {
        Err(_) => {
            error!("Pmatch: could not open text file {} for reading", filename);
        }
        Ok(contents) => {
            for line in contents.lines() {
                let line = line.to_string();
                if !line.is_empty() {
                    if spaced_text {
                        let _spv = HfstTokenizer::tokenize_space_separated(&line);
                    } else {
                        let spv = tok.tokenize(&line, false); // XXX
                        retval.disjunct_spv(&spv)?;
                    }
                }
            }
        }
    }
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
pub fn read_spaced_text<B: AlgebraBackend>(
    filename: String,
) -> crate::error::Result<HfstTransducer<B>> {
    read_text(filename, true)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.path-from-filename-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.path-from-filename-fn]
pub fn path_from_filename<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    filename: &str,
) -> String {
    let mut retval = filename.to_string();
    if !ctx.includedir.is_empty() && !retval.is_empty() {
        // includedir won't be > 0 under Windows until this mechanism is ported
        if retval.as_bytes()[0] != b'/' {
            // not an absolute dir
            retval.insert_str(0, &ctx.includedir.clone());
        }
    }
    retval
}

// [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.pmatch-compiler-fn]
// [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.pmatch-compiler-fn]
//
// The C++ default constructor fixed the format to TROPICAL_OPENFST_TYPE; the
// format is the type parameter 'B' now ([dec:hfst:monomorphic-backends]).
impl<B: AlgebraBackend + FromAnyTransducer + 'static> Default for PmatchCompiler<B> {
    fn default() -> Self {
        PmatchCompiler::new()
    }
}

impl<B: AlgebraBackend + FromAnyTransducer + 'static> PmatchCompiler<B> {
    pub fn new() -> Self {
        PmatchCompiler {
            verbose: false,
            flatten: false,
            include_cosine_distances: false,
            includedir: String::new(),
            definitions: BTreeMap::new(),
            eval_ctx: PmatchEvalContext::new(),
        }
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-flatten-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-flatten-fn]
    pub fn set_flatten(&mut self, val: bool) {
        self.flatten = val;
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-verbose-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-verbose-fn]
    pub fn set_verbose(&mut self, val: bool) {
        self.verbose = val;
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-cosine-distances-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-cosine-distances-fn]
    pub fn set_include_cosine_distances(&mut self, val: bool) {
        self.include_cosine_distances = val;
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.define-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.define-fn]
    //
    // Reads the global 'definitions' map (populated by 'compile') and stores the
    // evaluated transducer into the member 'definitions', mirroring the C++.
    pub fn define(&mut self, name: &str, pmatch: &str) -> crate::error::Result<()> {
        self.compile(pmatch)?;
        let ctx = &mut self.eval_ctx;
        if ctx.definitions_contains(name) {
            let obj = ctx
                .definitions_get(name)
                .expect("definitions_contains verified above");
            let evaluated = obj.evaluate(ctx)?;
            self.definitions.insert(name.to_string(), evaluated);
        }
        Ok(())
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.compile-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.compile-fn]
    // Mirrors 'hfst::pmatch::compile', with the bison 'pmatchparse()' step
    // replaced by a walk over the 'nfst-pmatch' AST (the sanctioned deviation).
    pub fn compile(
        &mut self,
        src: &str,
    ) -> crate::error::Result<HashMap<String, HfstTransducer<B>>> {
        {
            let ctx = &mut self.eval_ctx;
            ctx.init_globals();
            let expanded_script = expand_includes(ctx, src);
            ctx.verbose = self.verbose;
            ctx.flatten = self.flatten;
            ctx.include_cosine_distances = self.include_cosine_distances;
            ctx.includedir = self.includedir.clone();
            ctx.vector_similarity_projection_factor = 1.0;
            if ctx.verbose {
                ctx.timer = clock();
                debug!("");
            }

            // ---- bison-replacement walk ------------------------------------
            match nfst_pmatch::parse(&expanded_script) {
                Ok(file) => {
                    for stmt in &file.value.statements {
                        if let Err(e) = build_statement(ctx, stmt) {
                            error!("{}", e);
                            ctx.data = String::new();
                            ctx.len = 0;
                            return Ok(HashMap::new());
                        }
                    }
                }
                Err(e) => {
                    emit_pmatch_parse_error(&e, &expanded_script);
                    ctx.data = String::new();
                    ctx.len = 0;
                    return Ok(HashMap::new());
                }
            }

            let mut retval: HashMap<String, HfstTransducer<B>> = HashMap::new();

            for it in ctx.unsatisfied_insertions_snapshot().into_iter() {
                if !ctx.definitions_contains(it.as_str()) {
                    error!("Inserted transducer {} was never defined!", it);
                    ctx.data = String::new();
                    ctx.len = 0;
                    return Ok(retval);
                }
            }
            if ctx.verbose {
                for (k, _v) in ctx.definitions_snapshot() {
                    if !ctx.used_definitions_contains(&k) && k != "TOP" {
                        debug!("Warning: {} defined but never used", k);
                    }
                }
            }

            if ctx.verbose {
                debug!("Compiling and harmonizing...");
                ctx.timer = clock();
            }

            let mut uncount: u32 = 0;
            if !ctx.inserted_names_is_empty()
                || !ctx.def_insed_expressions_is_empty()
                || !ctx.uncomposed_is_empty()
            {
                let mut dummy = HfstTransducer::new();
                let keys: Vec<String> = ctx.definitions_keys();
                for key in &keys {
                    if key == "TOP"
                        || ctx.inserted_names_contains(key)
                        || ctx.def_insed_expressions_contains(key)
                        || ctx.uncomposed_contains(key)
                    {
                        let obj_ptr: ObjRef<B> = if ctx.def_insed_expressions_contains(key) {
                            ctx.def_insed_expressions_get(key)
                                .expect("checked with contains just above")
                        } else {
                            ctx.definitions_get(key).expect("key is a definitions key")
                        };
                        let mut tmp: HfstTransducer<B> = obj_ptr.evaluate(ctx)?;
                        tmp.minimize()?;
                        dummy.harmonize(&mut tmp, false)?;
                        if ctx.uncomposed_contains(key) {
                            if uncount == 0 {
                                tmp.set_name(&format!("UNCOMPOSE LEFT {}", key));
                                retval.insert(format!("UNCOMPOSE LEFT {}", key), tmp);
                                uncount += 1;
                            } else if uncount == 1 {
                                tmp.set_name(&format!("UNCOMPOSE RIGHT {}", key));
                                retval.insert(format!("UNCOMPOSE RIGHT {}", key), tmp);
                                uncount += 1;
                            } else {
                                warn!("Uncompose only works once so far...");
                                uncount += 1;
                            }
                        } else {
                            tmp.set_name(key);
                            retval.insert(key.clone(), tmp);
                        }
                    }
                }
                for v in retval.values_mut() {
                    v.harmonize(&mut dummy, false)?;
                    v.minimize()?;
                }
            } else if ctx.definitions_is_empty() {
                warn!("pmatch compilation had an empty result");
                retval.insert("TOP".to_string(), HfstTransducer::new());
            } else if !ctx.definitions_contains("TOP") {
                let (first_key, first_obj) = {
                    let snap = ctx.definitions_snapshot();
                    let mut it = snap.iter();
                    let (k, v) = it.next().expect("definitions non-empty in this branch");
                    (k.clone(), v.clone())
                };
                warn!(
                    "Pmatch compilation: regex or TOP was undefined, using {} as root",
                    first_key
                );
                let mut tmp: HfstTransducer<B> = first_obj.evaluate(ctx)?;
                tmp.minimize()?;
                tmp.set_name("TOP");
                retval.insert("TOP".to_string(), tmp);
            } else {
                let top_obj = ctx
                    .definitions_get("TOP")
                    .expect("TOP present in this branch");
                let mut tmp: HfstTransducer<B> = top_obj.evaluate(ctx)?;
                tmp.minimize()?;
                tmp.set_name("TOP");
                retval.insert("TOP".to_string(), tmp);
            }

            if ctx.verbose {
                let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
                ctx.timer = clock();
                debug!("Everything compiled and harmonized in {} seconds", duration);
            }

            let mut allowed_initial_symbols: StringSet = StringSet::new();
            let mut disallowed_initial_symbols: StringSet = StringSet::new();
            if let Some(top) = ctx.definitions_get("TOP") {
                top.collect_initial_symbols_into(
                    &mut allowed_initial_symbols,
                    &mut disallowed_initial_symbols,
                )?;
            }
            let mut initial_symbols_list = String::new();
            let mut disallowed_initial_symbols_list = String::new();
            let mut initial_symbols_ok = true;
            for it in allowed_initial_symbols.iter() {
                if is_special(it) {
                    if ctx.verbose {
                        debug!(
                            "Not setting initial symbol list due to special symbol {}",
                            it
                        );
                    }
                    initial_symbols_ok = false;
                }
                initial_symbols_list.push_str(it);
            }
            for it in disallowed_initial_symbols.iter() {
                if is_special(it) {
                    if ctx.verbose {
                        debug!(
                            "Not setting initial symbol list due to special symbol {}",
                            it
                        );
                    }
                    initial_symbols_ok = false;
                }
                disallowed_initial_symbols_list.push_str(it);
            }
            if allowed_initial_symbols.len() > 200 {
                if ctx.verbose {
                    debug!(
                        "Not setting initial symbol list due to excess length: {}",
                        allowed_initial_symbols.len()
                    );
                }
                initial_symbols_ok = false;
            }
            if disallowed_initial_symbols.len() > 200 {
                if ctx.verbose {
                    debug!(
                        "Not setting initial symbol list due to excess length: {}",
                        disallowed_initial_symbols.len()
                    );
                }
                initial_symbols_ok = false;
            }
            if initial_symbols_ok && !initial_symbols_list.is_empty() {
                ctx.variables_insert("initial-symbols".to_string(), initial_symbols_list);
            }
            if initial_symbols_ok && !disallowed_initial_symbols_list.is_empty() {
                ctx.variables_insert(
                    "disallowed-initial-symbols".to_string(),
                    disallowed_initial_symbols_list,
                );
            }

            if ctx.variables_get("need-separators").as_deref() == Some("on") {
                let whitespace_acc =
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_whitespace_acceptor))?;
                let punct_acc =
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_punct_acceptor))?;
                let mut not_whitespace =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
                not_whitespace.subtract(&whitespace_acc, true)?;
                let mut anything =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
                anything.repeat_star()?;
                let mut begins_and_ends_with_non_whitespace =
                    HfstTransducer::new_from_transducer(&not_whitespace);
                begins_and_ends_with_non_whitespace.concatenate(&anything, true)?;
                begins_and_ends_with_non_whitespace.concatenate(&not_whitespace, true)?;
                begins_and_ends_with_non_whitespace
                    .compose(retval.get("TOP").expect("TOP defined above"), true)?;
                let mut is_single_non_whitespace =
                    HfstTransducer::new_from_transducer(&not_whitespace);
                is_single_non_whitespace
                    .compose(retval.get("TOP").expect("TOP defined above"), true)?;
                let empty = HfstTransducer::new();
                if !begins_and_ends_with_non_whitespace.compare(&empty, true)?
                    || !is_single_non_whitespace.compare(&empty, true)?
                {
                    let mut whitespace_punct_context =
                        HfstTransducer::new_from_transducer(&whitespace_acc);
                    whitespace_punct_context.disjunct(&punct_acc, true)?;
                    whitespace_punct_context
                        .disjunct(&HfstTransducer::new_symbol("@BOUNDARY@")?, true)?;
                    let mut top_with_boundaries = HfstTransducer::new_symbol_pair(
                        crate::hfst_symbol_defs::internal_epsilon,
                        LC_ENTRY_SYMBOL,
                    )?;
                    top_with_boundaries.concatenate(&whitespace_punct_context, true)?;
                    top_with_boundaries.concatenate(
                        &HfstTransducer::new_symbol_pair(
                            crate::hfst_symbol_defs::internal_epsilon,
                            LC_EXIT_SYMBOL,
                        )?,
                        true,
                    )?;
                    let mut rc = HfstTransducer::new_symbol_pair(
                        crate::hfst_symbol_defs::internal_epsilon,
                        RC_ENTRY_SYMBOL,
                    )?;
                    rc.concatenate(&whitespace_punct_context, true)?;
                    rc.concatenate(
                        &HfstTransducer::new_symbol_pair(
                            crate::hfst_symbol_defs::internal_epsilon,
                            RC_EXIT_SYMBOL,
                        )?,
                        true,
                    )?;
                    top_with_boundaries
                        .concatenate(retval.get("TOP").expect("TOP defined above"), true)?;
                    top_with_boundaries.concatenate(&rc, true)?;
                    let mut new_top = add_pmatch_delimiters(&top_with_boundaries)?;
                    new_top.minimize()?;
                    retval.insert("TOP".to_string(), new_top);
                    if ctx.verbose {
                        let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
                        ctx.timer = clock();
                        debug!("Added automatic context separators in {} seconds", duration);
                    }
                }
            }

            let vars: Vec<(String, String)> = ctx.variables_snapshot();
            let top = retval
                .get_mut("TOP")
                .expect("TOP present in result by this point");
            for (k, v) in &vars {
                top.set_property(k, v);
            }
            ctx.data = String::new();
            ctx.len = 0;
            Ok(retval)
        }
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-path-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-path-fn]
    pub fn set_include_path(&mut self, path: String) {
        self.includedir = path;
    }
}

// ---------------------------------------------------------------------------
// Archive writing, lifted from tools/src/hfst-pmatch2fst.cc: turn a compiled
// ruleset (the definitions map from 'PmatchCompiler::compile') into a
// weighted optimized-lookup archive with every transducer harmonized against
// one shared alphabet, TOP first. The tool keeps its option parsing, stream
// opening and the includedir computation; verbose progress goes to a
// caller-supplied writer.
// ---------------------------------------------------------------------------

/// Build the shared harmonizer for a compiled ruleset: a transducer holding
/// the union of every definition's alphabet; ['write_archive'] turns it into
/// the optimized-lookup backend that harmonizes each archive member. Returns
/// None when the definitions carry no symbols at all (an empty ruleset).
pub fn build_archive_harmonizer<B: AlgebraBackend>(
    definitions: &HashMap<String, HfstTransducer<B>>,
) -> crate::error::Result<Option<HfstTransducer<B>>> {
    // A dummy transducer with an alphabet with all the symbols
    let mut harmonizer = HfstTransducer::new();
    // First we need to collect a unified alphabet from all the transducers.
    let mut symbols_seen: StringSet = BTreeSet::new();
    // Iterate in key order to mirror std::map's ordered iteration.
    let mut keys: Vec<&String> = definitions.keys().collect();
    keys.sort();
    for key in &keys {
        let t = &definitions[*key];
        let string_set = t.get_alphabet()?;
        for sym in string_set.iter() {
            if !symbols_seen.contains(sym) {
                harmonizer.insert_to_alphabet(sym)?;
                symbols_seen.insert(sym.clone());
            }
        }
    }
    if symbols_seen.is_empty() {
        // We don't recognise anything, go home early
        return Ok(None);
    }

    Ok(Some(harmonizer))
}

/// Write a compiled ruleset as a weighted optimized-lookup archive: TOP
/// first, then the remaining definitions in name order, each harmonized
/// against the shared alphabet from ['build_archive_harmonizer']. Returns
/// false — writing nothing — when the ruleset is empty (no symbols or no TOP
/// definition); the caller reports that in its own voice. Verbose progress
/// (with timings) goes to 'msg'.
pub fn write_archive<B: AlgebraBackend>(
    definitions: &mut HashMap<String, HfstTransducer<B>>,
    outstream: &mut crate::hfst_output_stream::HfstOutputStream,
    verbose: bool,
    msg: &mut dyn std::io::Write,
) -> crate::error::Result<bool> {
    use crate::convert_transducer_format::ConversionFunctions;

    let mut timer: clock_t = 0;
    if verbose {
        timer = clock();
        let _ = write!(msg, "Building hfst-ol alphabet... ");
    }

    let harmonizer = match build_archive_harmonizer(definitions)? {
        Some(h) => h,
        None => return Ok(false),
    };
    // Use these for naughty intermediate steps to make sure
    // everything has the same alphabet
    // C passed 'HfstTransducer* harmonizer' to the conversion functions,
    // which read its hfst_ol backend; the Rust conversion builds and returns
    // that weighted-OL backend as an owned value, so borrow it below.
    // The harmonizer holds only an alphabet (no transitions), so it is
    // converted with the 'harmonizer_alphabet' option: every symbol is numbered
    // as a potential input symbol, giving the shared header a correct
    // input_symbol_count. Without it each harmonized member's index table is
    // under-padded and the runtime crashes on an out-of-bounds lookup.
    // [upstream hfst/hfst#354]
    let harmonizer_basic =
        ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&harmonizer)?;
    let harmonizer_ol = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
        &harmonizer_basic,
        true, // weighted
        "harmonizer_alphabet",
        None,
    )?;

    if verbose {
        let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
        timer = clock();
        let _ = writeln!(msg, "built in {:.2} seconds", duration);
        let _ = write!(msg, "Converting TOP... ");
    }

    // When done compiling everything, look for TOP and output it first. TOP is
    // the multi-million-arc archive payload: convert it straight from the
    // backend (no basic-transducer intermediate) and drop the source before
    // wrapping, so only one large representation is live at a time — the old
    // tropical+basic+OL+copy chain owned the process's peak RSS.
    let Some(top) = definitions.remove("TOP") else {
        return Ok(false);
    };
    let properties: BTreeMap<String, String> = top.get_properties().clone();
    let harmonized_tmp = top.to_hfst_ol(
        true,                 // weighted
        "",                   // no special options
        Some(&harmonizer_ol), // harmonize with this
    )?;
    drop(top);
    let mut output_tmp = HfstTransducer::wrap(harmonized_tmp);
    output_tmp.set_name("TOP");
    for (k, v) in properties.iter() {
        output_tmp.set_property(k, v);
    }
    outstream.redirect(&mut output_tmp)?;

    if verbose {
        let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
        timer = clock();
        let _ = writeln!(msg, "converted in {:.2} seconds", duration);
    }

    let mut rest_keys: Vec<String> = definitions.keys().cloned().collect();
    rest_keys.sort();
    for key in &rest_keys {
        let t = &definitions[key];
        if verbose {
            let _ = writeln!(msg, "Converting {}... ", key);
            timer = clock();
        }
        let harmonized_tmp = if !key.contains("UNCOMPOSE") {
            t.to_hfst_ol(
                true,                 // weighted
                "empty_alphabet",     // empty alphabet in RTNs, they'll use the main one
                Some(&harmonizer_ol), // harmonize with this
            )
        } else {
            t.to_hfst_ol(
                true,                 // weighted
                "",                   // alphabet in UNCs,
                Some(&harmonizer_ol), // harmonize with this
            )
        };
        let harmonized_tmp = harmonized_tmp?;
        let mut output_tmp = HfstTransducer::wrap(harmonized_tmp);
        output_tmp.set_name(key);
        outstream.redirect(&mut output_tmp)?;
        if verbose {
            let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
            let _ = writeln!(msg, "converted in {:.2} seconds", duration);
        }
    }
    Ok(true)
}
