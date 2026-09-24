//! Alphabet-derived symbols ('Lst()', 'Exc()', sigma): construction, line
//! tracking, and overlap repair.

use super::*;

impl<B: AlgebraBackend + 'static> PmatchEvalContext<B> {
    // --- LST_LINE_MAP (BTreeMap<String, i32>) ---
    fn lst_line_map_contains(&self, k: &str) -> bool {
        self.lst_line_map.contains_key(k)
    }
    fn lst_line_map_get(&self, k: &str) -> Option<i32> {
        self.lst_line_map.get(k).copied()
    }
    fn lst_line_map_insert(&mut self, k: String, v: i32) {
        self.lst_line_map.insert(k, v);
    }
    fn lst_line_map_clear(&mut self) {
        self.lst_line_map.clear();
    }
    pub(super) fn lst_line_map_snapshot(&self) -> BTreeMap<String, i32> {
        self.lst_line_map.clone()
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-non-special-alphabet-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-non-special-alphabet-fn]
pub fn get_non_special_alphabet<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<StringSet> {
    let mut retval: StringSet = StringSet::new();
    let alphabet = t.get_alphabet()?;
    for it in alphabet.iter() {
        if PmatchAlphabet::is_printable(it) {
            retval.insert(it.clone());
        }
    }
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-list-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-list-fn]
pub fn make_list<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut transition = String::from("@L.");
    let alphabet = get_non_special_alphabet(t)?;
    for it in alphabet.iter() {
        transition.push_str(it);
        transition.push('_');
    }
    transition.push('@');
    HfstTransducer::new_symbol(&transition)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-exc-list-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-exc-list-fn]
pub fn make_exc_list<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut transition = String::from("@X.");
    let alphabet = get_non_special_alphabet(t)?;
    for it in alphabet.iter() {
        transition.push_str(it);
        transition.push('_');
    }
    transition.push('@');
    HfstTransducer::new_symbol(&transition)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-sigma-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-sigma-fn]
pub fn make_sigma<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval = HfstTransducer::new();
    let alphabet = get_non_special_alphabet(t)?;
    for it in alphabet.iter() {
        retval.disjunct(&HfstTransducer::new_symbol(it)?, true)?;
    }
    Ok(retval)
}

// Pass a map from Lst() symbol to line number. Emits one warning per Lst()
// symbol (so line numbers stay unambiguous).
// [spec:hfst:def:pmatch-utils.hfst.fix-list-overlap-fn]
// [spec:hfst:sem:pmatch-utils.hfst.fix-list-overlap-fn]
pub fn fix_list_overlap<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    lhs: &mut HfstTransducer<B>,
    rhs: &mut HfstTransducer<B>,
    list_set: &StringSet,
    literal_set: &StringSet,
    lst_line_map: &BTreeMap<String, i32>,
) -> crate::error::Result<()> {
    {
        for sym in list_set.iter() {
            if !sym.starts_with("@L.") {
                continue;
            }

            let mut overlapping_chars: Vec<String> = Vec::new();
            let mut retained_chars: Vec<String> = Vec::new();
            let mut lst_line: i32 = -1;
            if let Some(line) = lst_line_map.get(sym.as_str()) {
                lst_line = *line;
            }

            // Parse the list content: @L.a_b_c@ -> a, b, c
            let mut start: usize = 3; // Skip @L.
            let mut end: Option<usize> = sym[start..].find('_').map(|i| i + start);
            while let Some(e) = end {
                let sub = sym[start..e].to_string();
                if literal_set.contains(sub.as_str()) {
                    overlapping_chars.push(sub);
                } else {
                    retained_chars.push(sub);
                }
                start = e + 1;
                end = sym[start..].find('_').map(|i| i + start);
            }
            // Check last element (before final @)
            if start < sym.len() {
                let last = sym[start..sym.len() - 1].to_string();
                if literal_set.contains(last.as_str()) {
                    overlapping_chars.push(last);
                }
            }

            if overlapping_chars.is_empty() {
                continue;
            }

            // Ensure each Lst() only triggers a single warning per compilation.
            let warn_key: String = if lst_line >= 0 {
                let mut wk = lst_line.to_string();
                wk.push('\t');
                wk.push_str(sym);
                wk
            } else {
                sym.to_string()
            };
            if ctx.lst_overlap_warned_contains(&warn_key) {
                continue;
            }
            ctx.lst_overlap_warned_insert(warn_key);
            let mut newlist = String::from("@L.");
            for s in retained_chars.iter() {
                newlist.push_str(s);
                newlist.push('_');
            }
            newlist.push('@');
            let newsym: StringPair = (Symbol::from(newlist.clone()), Symbol::from(newlist.clone()));
            let mut newpairs: StringPairSet = StringPairSet::new();
            newpairs.insert(newsym);
            let mut optimise_msg = String::new();
            if ctx.verbose {
                optimise_msg.push_str(&format!(
                    "Automatically optimising:Removing the following symbols from Lst() (line {}):",
                    if lst_line >= 0 {
                        lst_line.to_string()
                    } else {
                        "?".to_string()
                    }
                ));
            }
            for overlapping_char in overlapping_chars.iter() {
                let overlapsym: StringPair = (
                    Symbol::from(overlapping_char.clone()),
                    Symbol::from(overlapping_char.clone()),
                );
                if ctx.verbose {
                    optimise_msg.push_str(&format!("\n  '{}' (", overlapping_char));
                    let mut buf: Vec<u8> = Vec::new();
                    print_unicode_codepoints(&mut buf, overlapping_char);
                    optimise_msg.push_str(&String::from_utf8_lossy(&buf));
                    optimise_msg.push(')');
                }
                newpairs.insert(overlapsym);
            }
            if ctx.verbose {
                debug!("{}", optimise_msg);
                debug!(
                    "Replacing all {} instances with new list: {} and abovementioned disjunction ",
                    sym, newlist
                );
            }
            let oldsym: StringPair = (sym.clone(), sym.clone());
            lhs.substitute_pair_with_pair_set(&oldsym, &newpairs)?;
            rhs.substitute_pair_with_pair_set(&oldsym, &newpairs)?;
        }
    }
    Ok(())
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.register-lst-line-numbers-from-transducer-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.register-lst-line-numbers-from-transducer-fn]
pub fn register_lst_line_numbers_from_transducer<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    t: &HfstTransducer<B>,
    line: i32,
) -> crate::error::Result<()> {
    if line <= 0 {
        return Ok(());
    }
    let ss: StringSet = t.get_alphabet()?;
    for it in ss.iter() {
        if it.find("@L.") == Some(0) {
            // Keep first occurrence if seen before.
            if !ctx.lst_line_map_contains(it) {
                ctx.lst_line_map_insert(it.to_string(), line);
            }
        }
    }
    Ok(())
}

// [spec:hfst:def:pmatch-utils.hfst.print-unicode-codepoints-fn]
// [spec:hfst:sem:pmatch-utils.hfst.print-unicode-codepoints-fn]
pub fn print_unicode_codepoints(os: &mut dyn std::io::Write, s: &str) {
    let bytes = s.as_bytes();
    let mut i: usize = 0;
    while i < bytes.len() {
        let c: u8 = bytes[i];
        let codepoint: u32;
        let clen: usize;
        if (c & 0x80) == 0 {
            codepoint = c as u32;
            clen = 1;
        } else if (c & 0xE0) == 0xC0 {
            codepoint = (((c & 0x1F) as u32) << 6) | ((bytes[i + 1] & 0x3F) as u32);
            clen = 2;
        } else if (c & 0xF0) == 0xE0 {
            codepoint = (((c & 0x0F) as u32) << 12)
                | (((bytes[i + 1] & 0x3F) as u32) << 6)
                | ((bytes[i + 2] & 0x3F) as u32);
            clen = 3;
        } else if (c & 0xF8) == 0xF0 {
            codepoint = (((c & 0x07) as u32) << 18)
                | (((bytes[i + 1] & 0x3F) as u32) << 12)
                | (((bytes[i + 2] & 0x3F) as u32) << 6)
                | ((bytes[i + 3] & 0x3F) as u32);
            clen = 4;
        } else {
            codepoint = c as u32;
            clen = 1;
        }
        let _ = write!(os, "U+{:04X}", codepoint);
        i += clen;
        if i < bytes.len() {
            let _ = write!(os, ", ");
        }
    }
}
