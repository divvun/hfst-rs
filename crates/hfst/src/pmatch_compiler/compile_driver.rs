//! The compile entry points, include expansion, file reading, and archive writing.

use super::*;
use crate::diag::Diagnostic;

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
/// The deepest '@include' nesting allowed. Without a limit a file that
/// includes itself would overflow the stack.
const MAX_INCLUDE_DEPTH: u32 = 64;

// [spec:hfst:def:pmatch-utils.hfst.pmatch.expand-includes-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.expand-includes-fn]
// [spec:hfst:req:javascript.named-file-resolver]
/// `script` with each '@include"file"' outside a literal or comment replaced
/// by the file's contents, read through the context's file resolver. An
/// included file's own includes are expanded the same way.
pub fn expand_includes<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    script: &str,
) -> crate::error::Result<String> {
    if !script.contains("@include\"") {
        return Ok(script.to_string());
    }
    let mut retval: Vec<u8> = Vec::new();
    expand_includes_into(ctx, script.as_bytes(), 0, &mut retval)?;
    Ok(String::from_utf8_lossy(&retval).into_owned())
}

// The scan works on bytes so that text outside the directives is copied
// unchanged, whatever its encoding.
fn expand_includes_into<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    bytes: &[u8],
    depth: u32,
    retval: &mut Vec<u8>,
) -> crate::error::Result<()> {
    let mut in_quoted_literal = false;
    let mut in_curly_literal = false;
    let mut in_comment = false;
    let mut idx: usize = 0;
    while idx < bytes.len() {
        let c = bytes[idx];
        if in_quoted_literal && c == b'"' && (idx == 0 || bytes[idx - 1] != b'\\') {
            in_quoted_literal = false;
        } else if in_curly_literal && c == b'}' && (idx == 0 || bytes[idx - 1] != b'\\') {
            in_curly_literal = false;
        } else if in_comment && c == b'\n' {
            in_comment = false;
        } else if c == b'"' {
            in_quoted_literal = true;
        } else if c == b'{' {
            in_curly_literal = true;
        } else if c == b'!' {
            in_comment = true;
        } else if c == b'%' {
            retval.push(c);
            idx += 1;
            if idx < bytes.len() {
                retval.push(bytes[idx]);
                idx += 1;
            }
            continue;
        } else if bytes[idx..].starts_with(b"@include\"") {
            let filename_start_pos = idx + 9;
            if let Some(filename_len) = bytes[filename_start_pos..].iter().position(|&b| b == b'"')
            {
                let filename = String::from_utf8_lossy(
                    &bytes[filename_start_pos..filename_start_pos + filename_len],
                );
                let filepath = path_from_filename(ctx, &filename);
                if depth >= MAX_INCLUDE_DEPTH {
                    crate::bail!(
                        Hfst,
                        format!(
                            "'@include' of {} is nested more than {} deep; does a file include itself?",
                            filepath, MAX_INCLUDE_DEPTH
                        )
                    );
                }
                let contents = ctx.files.read(&filepath).map_err(|_| {
                    crate::err!(
                        Hfst,
                        format!("could not open file {} for @include", filepath)
                    )
                })?;
                expand_includes_into(ctx, &contents, depth + 1, retval)?;
                idx += 10 + filename_len;
                continue;
            }
        }
        retval.push(c);
        idx += 1;
    }
    Ok(())
}
// Record a parser failure as source-anchored diagnostics, so the real cause
// reaches the caller, e.g. a reserved predefined acceptor name (`Alpha`,
// `Whitespace`, ...) used as a `Define` target. Each diagnostic carries a byte
// span into the include-expanded script, which is the source it points into.
fn report_pmatch_parse_error<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    e: &nfst_pmatch::ParseError,
    expanded_script: &str,
) {
    let source: Arc<str> = Arc::from(expanded_script);
    if e.diagnostics.is_empty() {
        ctx.diagnostics
            .push(Diagnostic::error("pmatch: syntax error").in_source("<pmatch>", source.clone()));
    }
    for d in &e.diagnostics {
        let message = format!("pmatch: syntax error: {}", d.message);
        ctx.diagnostics.push(
            Diagnostic::error(&message)
                .label(d.span.range.clone(), message)
                .in_source("<pmatch>", source.clone()),
        );
    }
}

/// The error a script that does not parse fails with. Its diagnostics carry
/// the detail; this names how many there were.
fn syntax_error(e: &nfst_pmatch::ParseError) -> crate::error::Error {
    crate::err!(
        Hfst,
        format!(
            "pmatch: the script has {} syntax error(s)",
            e.diagnostics.len().max(1)
        )
    )
}

/// Fail when an `Ins()` names a definition the script never makes: the
/// runtime would have nothing to insert there.
fn check_insertions_defined<B: AlgebraBackend + 'static>(
    ctx: &PmatchEvalContext<B>,
) -> crate::error::Result<()> {
    for it in ctx.unsatisfied_insertions_snapshot() {
        if !ctx.definitions_contains(it.as_str()) {
            crate::bail!(
                Hfst,
                format!("pmatch: inserted transducer {} was never defined", it)
            );
        }
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
// [spec:hfst:req:javascript.named-file-resolver]
pub fn read_text<B: AlgebraBackend>(
    files: &dyn FileResolver,
    filename: String,
    spaced_text: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let tok = HfstTokenizer::new();
    let mut retval: HfstTransducer<B> = HfstTransducer::new();
    let contents = files.read_to_string(&filename).map_err(|_| {
        crate::err!(
            Hfst,
            format!("Pmatch: could not open text file {} for reading", filename)
        )
    })?;
    for line in contents.lines() {
        let line = line.to_string();
        if !line.is_empty() {
            if spaced_text {
                let _spv = HfstTokenizer::tokenize_space_separated(&line);
            } else {
                let spv = tok.tokenize(&line, false);
                retval.disjunct_spv(&spv)?;
            }
        }
    }
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
pub fn read_spaced_text<B: AlgebraBackend>(
    files: &dyn FileResolver,
    filename: String,
) -> crate::error::Result<HfstTransducer<B>> {
    read_text(files, filename, true)
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
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-fn]
    // Mirrors 'hfst::pmatch::compile', with the bison 'pmatchparse()' step
    // replaced by a walk over the 'nfst-pmatch' AST (the sanctioned deviation).
    pub fn compile(
        &mut self,
        src: &str,
    ) -> crate::error::Result<HashMap<String, HfstTransducer<B>>> {
        {
            let ctx = &mut self.eval_ctx;
            ctx.init_globals();
            let expanded_script = expand_includes(ctx, src)?;
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
                            ctx.data = String::new();
                            ctx.len = 0;
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    report_pmatch_parse_error(ctx, &e, &expanded_script);
                    ctx.data = String::new();
                    ctx.len = 0;
                    return Err(syntax_error(&e));
                }
            }

            if let Err(e) = check_insertions_defined(ctx) {
                ctx.data = String::new();
                ctx.len = 0;
                return Err(e);
            }
            let mut retval: HashMap<String, HfstTransducer<B>> = HashMap::new();
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
                    HfstTransducer::new_copy(&not_whitespace)?;
                begins_and_ends_with_non_whitespace.concatenate(&anything, true)?;
                begins_and_ends_with_non_whitespace.concatenate(&not_whitespace, true)?;
                begins_and_ends_with_non_whitespace
                    .compose(retval.get("TOP").expect("TOP defined above"), true)?;
                let mut is_single_non_whitespace = HfstTransducer::new_copy(&not_whitespace)?;
                is_single_non_whitespace
                    .compose(retval.get("TOP").expect("TOP defined above"), true)?;
                let empty = HfstTransducer::new();
                if !begins_and_ends_with_non_whitespace.compare(&empty, true)?
                    || !is_single_non_whitespace.compare(&empty, true)?
                {
                    let mut whitespace_punct_context = HfstTransducer::new_copy(&whitespace_acc)?;
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

    // [spec:hfst:req:javascript.named-file-resolver]
    /// The resolver that supplies the files named by includes and file
    /// references, with the include path prefixed to a relative name. The
    /// default reads the filesystem.
    pub fn set_file_resolver(&mut self, files: Arc<dyn FileResolver>) {
        self.eval_ctx.files = files;
    }

    /// Everything compiling has reported since the diagnostics were last
    /// taken, oldest first, failures and warnings alike.
    // [spec:hfst:req:javascript.diagnostics-as-data]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.eval_ctx.diagnostics
    }

    /// Hand over the diagnostics reported so far, leaving none behind.
    // [spec:hfst:req:javascript.diagnostics-as-data]
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.eval_ctx.diagnostics)
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
                harmonizer.insert_to_alphabet_string(sym)?;
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
    outstream.write(&mut output_tmp)?;

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
        outstream.write(&mut output_tmp)?;
        if verbose {
            let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
            let _ = writeln!(msg, "converted in {:.2} seconds", duration);
        }
    }
    Ok(true)
}
