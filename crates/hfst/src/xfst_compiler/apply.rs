//! Lookup: apply up, apply down and apply med, and lookup optimization.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

static APPLY_END_STRING: &str = "<ctrl-d>";

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // @brief Perform lookdowns on top of the stack, one per line
    // @todo lookdown is missing from HFST
    pub fn apply_up(&mut self, indata: &str) -> CmdResult {
        for line in indata.split('\n').filter(|s| !s.is_empty()) {
            if line == APPLY_END_STRING {
                break;
            }
            self.apply_up_line(line)?;
        }
        self.prompt();
        Ok(())
    }

    // @brief Perform lookups on top of the stack, one per line
    // @todo lookup is missing from HFST
    pub fn apply_down(&mut self, indata: &str) -> CmdResult {
        for line in indata.split('\n').filter(|s| !s.is_empty()) {
            if line == APPLY_END_STRING {
                break;
            }
            self.apply_down_line(line)?;
        }
        self.prompt();
        Ok(())
    }

    // @brief Perform lookmeds on top of the stack, one per line
    // @todo lookmed is missing from HFST
    pub fn apply_med(&mut self, _indata: &str) -> CmdResult {
        Err(CommandError::not_supported("apply med"))
    }

    pub fn lookup_optimize(&mut self) -> CmdResult {
        self.top()?;
        Err(CommandError::new(
            "'lookup-optimize' is not supported: the stack holds one network type, and optimized lookup is a different one",
        )
        .with_note("'hfst fst2fst -O' converts a saved network to optimized lookup"))
    }

    pub fn remove_optimization(&mut self) -> CmdResult {
        self.top()?;
        self.diag_warning("network is already in ordinary format");
        self.prompt();
        Ok(())
    }

    // (The former OL-only 'fn lookup' is gone: its 'lookup_fd_string'
    // surface exists only on the OL instantiations, which
    // 'B: AlgebraBackend' excludes; every apply path uses 'lookup_basic'.)

    fn lookup_basic(&mut self, line: &str, t: &HfstBasicTransducer) {
        let token = trim_whitespace(line);

        let alpha = t.get_input_symbols();
        let mut tok = crate::hfst_tokenizer::HfstTokenizer::new();
        for it in alpha.iter() {
            tok.add_multichar_symbol(it);
        }
        let lookup_path: Vec<Symbol> = tok.tokenize_one_level(&token, false);

        let mut cutoff: usize = usize::MAX;
        if t.is_lookup_infinitely_ambiguous_string_vector(
            &lookup_path,
            self.variables["obey-flags"] == "ON",
        ) {
            cutoff = parse_size(&self.variables["lookup-cycle-cutoff"]);
            if self.verbose {
                self.diag_warning(&format!(
                    "lookup is infinitely ambiguous, limiting the number of cycles to {}",
                    cutoff
                ));
            }
        }

        let mut results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        let max_weight = (self.variables["maximum-weight"] != "OFF")
            .then(|| string_to_float(&self.variables["maximum-weight"]));
        t.lookup(
            &lookup_path,
            &mut results,
            Some(cutoff),
            max_weight.as_ref(),
            -1, /*max_number*/
            self.variables["obey-flags"] == "ON",
        );

        let mut out = std::io::stdout();
        let printed = if self.variables["print-pairs"] == "OFF" {
            let paths = extract_output_paths(&results);
            self.print_paths_one(&paths, &mut out, -1)
        } else {
            self.print_paths_two(&results, &mut out, -1)
        };

        if !printed {
            println!("???");
        }
    }

    // apply_down_line -> apply_up_line
    fn apply_up_line(&mut self, line: &str) -> CmdResult {
        let t = self.top()?;
        if self.verbose {
            self.diag_warning(
                "apply up on this format is done by inverting and applying down; invert and minimize the top network yourself to avoid the cost",
            );
        }
        let mut copy = HfstTransducer::new_copy(self.net(t))?;
        copy.invert()?.minimize_with_config(&self.engine_config)?;
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&copy)?;
        self.lookup_basic(line, &fsm);
        Ok(())
    }

    // apply_up_line -> apply_down_line
    fn apply_down_line(&mut self, line: &str) -> CmdResult {
        let t = self.top()?;
        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(t))?;
        self.lookup_basic(line, &fsm);
        Ok(())
    }
}

fn trim_whitespace(s: &str) -> String {
    let bytes = s.as_bytes();
    let is_space = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    let mut start = 0;
    while start < bytes.len() && is_space(bytes[start]) {
        start += 1;
    }
    let mut end = bytes.len();
    while end > start && is_space(bytes[end - 1]) {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.string-to-float-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.string-to-float-fn]
fn string_to_float(str: &str) -> f32 {
    // Mirror 'std::istringstream >> float': skip leading whitespace, read the
    // leading numeric prefix, and yield 0 if nothing parses.
    let t = str.trim_start();
    let bytes = t.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        let mut e = end + 1;
        if e < bytes.len() && (bytes[e] == b'+' || bytes[e] == b'-') {
            e += 1;
        }
        let mut saw = false;
        while e < bytes.len() && bytes[e].is_ascii_digit() {
            e += 1;
            saw = true;
        }
        if saw {
            end = e;
        }
    }
    t[..end].parse::<f32>().unwrap_or(0.0)
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.extract-output-paths-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-output-paths-fn]
fn extract_output_paths(paths: &HfstTwoLevelPaths) -> HfstOneLevelPaths {
    let mut retval = HfstOneLevelPaths::new();
    for it in paths.iter() {
        let mut new_path: Vec<Symbol> = Vec::new();
        let path = &it.second;
        for p in path.iter() {
            if p.1 != "@0@" && p.1 != "@_EPSILON_SYMBOL_@" {
                if p.1 == "@_UNKNOWN_SYMBOL_@" {
                    new_path.push(Symbol::new_static("?"));
                } else {
                    new_path.push(p.1.clone());
                }
            }
        }
        retval.insert(crate::hfst_data_types::HfstOneLevelPath {
            first: it.first,
            second: new_path,
        });
    }
    retval
}
