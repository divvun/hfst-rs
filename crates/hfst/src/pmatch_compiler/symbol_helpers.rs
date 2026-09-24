//! Special-symbol helpers: definition lookup, tags, context markers, and ranges.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

// [spec:hfst:def:pmatch-utils.hfst.pmatch.add-to-pmatch-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-to-pmatch-symbols-fn]
pub fn add_to_pmatch_symbols(symbols: StringSet) {
    // Declared in pmatch_utils.h but never defined in pmatch_utils.cc.
}

// Helper for faithfully constructing a freshly-'new'ed PmatchObject node's base
// fields, mirroring the C++ 'PmatchObject::PmatchObject()' default constructor
// (name="", weight=0.0, line_defined=pmatchlineno, my_timer uninitialised,
// cache=NULL). There is no lexer line counter in this port, so line_defined=0.

// ---- libc-free C-string helpers for the surviving char*-based pmatch utils ----

// [spec:hfst:def:pmatch-utils.should-colourise-fn]
// [spec:hfst:sem:pmatch-utils.should-colourise-fn]
pub fn should_colourise() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.warn-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.warn-fn]
pub fn warn(warning: String) {
    warn!("hfst-pmatch: {}", warning);
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-in-local-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-in-local-context-fn]
pub fn symbol_in_local_context<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    sym: &str,
) -> bool {
    if ctx.call_stack_len() == 0 {
        return false;
    }
    ctx.call_stack_last_get(sym).is_some()
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-from-global-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-from-global-context-fn]
//
// Returns the shared AST node bound to 'sym' in the global definitions. The
// callers always guard with 'definitions_contains', so the C++ NULL branch
// is unreachable.
pub fn symbol_from_global_context<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    sym: &str,
) -> Option<ObjRef<B>> {
    if ctx.definitions_contains(sym) {
        ctx.definitions_get(sym)
    } else {
        None
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-from-local-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-from-local-context-fn]
pub fn symbol_from_local_context<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    sym: &str,
) -> Option<ObjRef<B>> {
    if symbol_in_local_context(ctx, sym) {
        ctx.call_stack_last_get(sym)
    } else {
        None
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.string-set-has-meta-arc-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.string-set-has-meta-arc-fn]
pub fn string_set_has_meta_arc(ss: &mut StringSet) -> bool {
    ss.iter().filter(|s| s.as_str() == internal_unknown).count() == 1
        || ss
            .iter()
            .filter(|s| s.as_str() == internal_identity)
            .count()
            == 1
        || ss.iter().filter(|s| s.as_str() == internal_default).count() == 1
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.is-special-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.is-special-fn]
pub fn is_special(symbol: &str) -> bool {
    if symbol.len() < 3 {
        return false;
    }
    symbol.find('@') == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-ins-transition-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-ins-transition-fn]
pub fn get_Ins_transition(s: &str) -> String {
    format!("@I.{}@", s)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.add-pmatch-delimiters-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-pmatch-delimiters-fn]
pub fn add_pmatch_delimiters<B: AlgebraBackend>(
    regex: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut delimited_regex = HfstTransducer::new_symbol_pair(internal_epsilon, ENTRY_SYMBOL)?;
    delimited_regex.concatenate(regex, true)?;
    let exit = HfstTransducer::new_symbol_pair(internal_epsilon, EXIT_SYMBOL)?;
    delimited_regex.concatenate(&exit, true)?;
    Ok(delimited_regex)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-end-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-end-tag-fn]
pub fn make_end_tag<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    tag: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, format!("@PMATCH_ENDTAG_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-capture-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-capture-tag-fn]
pub fn make_capture_tag<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    tag: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, format!("@PMATCH_CAPTURE_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-captured-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-captured-tag-fn]
pub fn make_captured_tag<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    tag: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, format!("@PMATCH_CAPTURED_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-with-tag-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-with-tag-entry-fn]
pub fn make_with_tag_entry<B: AlgebraBackend + 'static>(key: String, value: String) -> ObjRef<B> {
    let obj = PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        string: Symbol::from(format!("@P.PMATCH_GLOBAL_{}.{}@", key, value)),
        multichar: false,
        _marker: std::marker::PhantomData,
    };
    as_obj(Rc::new(obj))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-with-tag-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-with-tag-exit-fn]
pub fn make_with_tag_exit<B: AlgebraBackend + 'static>(key: String) -> ObjRef<B> {
    let obj = PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        string: Symbol::from(format!("@C.PMATCH_GLOBAL_{}@", key)),
        multichar: false,
        _marker: std::marker::PhantomData,
    };
    as_obj(Rc::new(obj))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-counter-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-counter-fn]
pub fn make_counter<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    name: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, format!("@PMATCH_COUNTER_{}@", name))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.epsilon-to-symbol-container-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.epsilon-to-symbol-container-fn]
pub fn epsilon_to_symbol_container<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    s: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    let tmp = HfstTransducer::new_symbol_pair(internal_epsilon, &s)?;
    let container = PmatchTransducerContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        t: tmp,
    };
    Ok(Rc::new(container))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-rc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-rc-entry-fn]
pub fn make_rc_entry<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, RC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-lc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-lc-entry-fn]
pub fn make_lc_entry<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, LC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nrc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nrc-entry-fn]
pub fn make_nrc_entry<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, NRC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nlc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nlc-entry-fn]
pub fn make_nlc_entry<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, NLC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-rc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-rc-exit-fn]
pub fn make_rc_exit<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, RC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-lc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-lc-exit-fn]
pub fn make_lc_exit<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, LC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nrc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nrc-exit-fn]
pub fn make_nrc_exit<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, NRC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nlc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nlc-exit-fn]
pub fn make_nlc_exit<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, NLC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-passthrough-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-passthrough-fn]
pub fn make_passthrough<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, PASSTHROUGH_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-delimited-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-delimited-fn]
pub fn get_delimited_lr(s: &str, delim_left: char, delim_right: char) -> String {
    // The content between the first 'delim_left' and the last 'delim_right'.
    let start = match s.find(delim_left) {
        Some(i) => i + delim_left.len_utf8(),
        None => return String::new(),
    };
    let end = s.rfind(delim_right).unwrap_or(s.len());
    if end <= start {
        String::new()
    } else {
        s[start..end].to_string()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.codepoint-to-utf8-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.codepoint-to-utf8-fn]
pub fn codepoint_to_utf8(codepoint: u32) -> String {
    // Invalid codepoints (surrogates, > U+10FFFF) yield the empty string, as
    // the C 'u_parse_err' path did. So does NUL: the original wrote it into a
    // C buffer whose 'std::string(buf)' constructor truncated at the NUL.
    match char::from_u32(codepoint) {
        None | Some('\0') => String::new(),
        Some(c) => String::from(c),
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.parse-range-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.parse-range-fn]
pub fn parse_range<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    s: &str,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    // Reads one codepoint at the cursor: a '\uXXXX' / '\UXXXXXXXX' hex escape, or
    // the next UTF-8 character. Advances the byte cursor past what it consumed.
    fn read_codepoint(bytes: &[u8], quoted: &str, i: &mut usize) -> u32 {
        if bytes.len() - *i >= 6
            && bytes[*i] == b'\\'
            && (bytes[*i + 1] == b'u' || bytes[*i + 1] == b'U')
        {
            let width = if bytes[*i + 1] == b'u' { 6 } else { 10 };
            let end = (*i + width).min(bytes.len());
            let hex = &quoted[*i + 2..end];
            *i += width;
            u32::from_str_radix(hex, 16).unwrap_or(0)
        } else {
            let ch = quoted[*i..]
                .chars()
                .next()
                .expect("index within bounds of the quoted string");
            *i += ch.len_utf8();
            ch as u32
        }
    }
    let quoted = get_delimited_lr(s, '"', '"');
    let bytes = quoted.as_bytes();
    let mut i = 0usize;
    let mut retval = HfstTransducer::new();
    while i < bytes.len() {
        let mut codepoint1 = read_codepoint(bytes, &quoted, &mut i);
        if i >= bytes.len() || bytes[i] != b'-' {
            ctx.pmatcherror(&format!("Could not parse range expression: {}", s));
        }
        i += 1;
        let codepoint2 = read_codepoint(bytes, &quoted, &mut i);
        if codepoint1 == 0 || codepoint2 == 0 {
            ctx.pmatcherror(&format!("Malformed character in range expression: {}", s));
        }
        if codepoint2 < codepoint1 {
            ctx.pmatcherror(&format!(
                "Range expression goes from higher to lower: {}",
                s
            ));
        }
        while codepoint1 <= codepoint2 {
            retval.disjunct(
                &HfstTransducer::new_symbol(&codepoint_to_utf8(codepoint1))?,
                true,
            )?;
            codepoint1 += 1;
        }
    }
    let container = PmatchTransducerContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        t: retval,
    };
    Ok(Rc::new(container))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-size-info-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-size-info-fn]
pub fn get_size_info<B: AlgebraBackend>(net: &HfstTransducer<B>) -> String {
    let tmp = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(net)
        .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
    let mut states: usize = 0;
    let mut arcs: usize = 0;
    for state_it in tmp.states_and_transitions().iter() {
        states += 1;
        for _tr_it in state_it.iter() {
            arcs += 1;
        }
    }
    format!("{} states and {} arcs", states, arcs)
}
