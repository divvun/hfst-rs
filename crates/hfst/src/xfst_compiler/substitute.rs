//! The substitute commands: symbols, labels, and defined networks.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    pub fn substitute_named(
        &mut self,
        variable: &str,
        label: &str,
    ) -> crate::error::Result<&mut Self> {
        // GET_TOP(top)
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        if !self.definitions.contains_key(variable) {
            self.diag_unknown_definition(variable);
            // MAYBE_QUIT
            if self.variables["quit-on-fail"] == "ON" {
                self.fail_flag = true;
            }
            self.prompt();
            return Ok(self);
        }
        let def_ptr = self.definitions[variable];

        // [spec:hfst:def:xfst-compiler.hfst.xfst.labelstr-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.labelstr-fn]
        let mut labelstr = Symbol::new(label);
        if labelstr == "?" {
            labelstr = Symbol::new_static("@_IDENTITY_SYMBOL_@");
        }
        if labelstr == "0" {
            labelstr = Symbol::new_static("@_EPSILON_SYMBOL_@");
        }

        let mut alpha = self.net(top).get_alphabet()?;
        if !alpha.contains(&labelstr) {
            self.diag_error(&format!(
                "no occurrences of label '{}' in the network, nothing to substitute",
                label
            ));
            // MAYBE_QUIT
            if self.variables["quit-on-fail"] == "ON" {
                self.fail_flag = true;
            }
            self.prompt();
            return Ok(self);
        }

        let fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(top))?;

        for it in fsm.iter() {
            for tr_it in it {
                let isymbol = tr_it.get_input_symbol(fsm.coder());
                let osymbol = tr_it.get_output_symbol(fsm.coder());
                if isymbol != osymbol && (isymbol == labelstr || osymbol == labelstr) {
                    self.diag_error(&format!(
                        "label '{}' is used as a symbol on one side of an arc, so it cannot be substituted",
                        label
                    ));
                    // MAYBE_QUIT
                    if self.variables["quit-on-fail"] == "ON" {
                        self.fail_flag = true;
                    }
                    self.prompt();
                    return Ok(self);
                }
            }
        }

        // [spec:hfst:def:xfst-compiler.hfst.xfst.labelpair-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.labelpair-fn]
        let labelpair: StringPair = (labelstr.clone(), labelstr.clone());
        alpha = self.net(def_ptr).get_alphabet()?;
        let (top_m, def_m) = self.net_pair_mut(top, def_ptr);
        top_m.substitute_pair_with_transducer(&labelpair, def_m, false)?;

        if labelstr != "@_EPSILON_SYMBOL_@"
            && labelstr != "@_IDENTITY_SYMBOL_@"
            && !alpha.contains(&labelstr)
        {
            self.net_mut(top).remove_from_alphabet_string(&labelstr)?;
        }

        // MAYBE_MINIMIZE(top)
        let cfg = self.engine_config;
        self.net_mut(top).optimize_with_config(&cfg)?;
        self.prompt();
        Ok(self)
    }

    // @brief Substitute all labels @a list by @a target.
    pub fn substitute_label(
        &mut self,
        list: &str,
        target: &str,
    ) -> crate::error::Result<&mut Self> {
        // GET_TOP(top)
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        // tokenize list into labels
        let mut symbol_pairs: StringPairSet = StringPairSet::new();

        if list != "NOTHING" {
            let labels = Self::tokenize_string(list, ' ');
            for label in labels.iter() {
                // tokenize labels into string pairs
                let sv = Self::tokenize_string(label, ':');
                match Self::symbol_vector_to_symbol_pair(&sv) {
                    Some(sp) => {
                        symbol_pairs.insert(sp);
                    }
                    None => {
                        self.diag_error(&format!("could not substitute with '{}'", list));
                        // MAYBE_QUIT
                        if self.variables["quit-on-fail"] == "ON" {
                            self.fail_flag = true;
                        }
                        self.prompt();
                        return Ok(self);
                    }
                }
            }
        }

        // tokenize target label into string pair
        let target_vector = Self::tokenize_string(target, ':');
        match Self::symbol_vector_to_symbol_pair(&target_vector) {
            Some(target_label) => {
                let fsm =
                    ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(top))?;
                let mut target_label_found = false;

                for it in fsm.iter() {
                    if target_label_found {
                        break;
                    }
                    for tr_it in it {
                        if target_label.0 == tr_it.get_input_symbol(fsm.coder())
                            && target_label.1 == tr_it.get_output_symbol(fsm.coder())
                        {
                            target_label_found = true;
                            break;
                        }
                    }
                }
                if !target_label_found {
                    self.diag_error(&format!(
                        "no occurrences of '{}:{}' in the network, nothing to substitute",
                        target_label.0, target_label.1
                    ));
                    self.prompt();
                    return Ok(self);
                }

                self.net_mut(top)
                    .substitute_pair_with_pair_set(&target_label, &symbol_pairs)?;
            }
            None => {
                self.diag_error(&format!("could not substitute '{}'", target));
                // MAYBE_QUIT
                if self.variables["quit-on-fail"] == "ON" {
                    self.fail_flag = true;
                }
            }
        }

        // MAYBE_MINIMIZE(top)
        let cfg = self.engine_config;
        self.net_mut(top).optimize_with_config(&cfg)?;
        self.prompt();
        Ok(self)
    }

    // @brief Substitute all symbols in @a list by @a target.
    pub fn substitute_symbol(
        &mut self,
        list: &str,
        target: &str,
    ) -> crate::error::Result<&mut Self> {
        // GET_TOP(top)
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        let alpha = self.net(top).get_alphabet()?;
        if !alpha.contains(target) {
            self.diag_error(&format!(
                "no occurrences of symbol '{}' in the network, nothing to substitute",
                target
            ));
            // MAYBE_QUIT
            if self.variables["quit-on-fail"] == "ON" {
                self.fail_flag = true;
            }
            self.prompt();
            return Ok(self);
        }

        self.stack.pop();

        // [spec:hfst:def:xfst-compiler.hfst.xfst.liststr-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.liststr-fn]
        let mut liststr = list.to_string();
        if liststr == "\"NOTHING\"" {
            // list is given in quoted format: "foo" "bar" ...
            liststr = String::new();
        }

        // use regex parser to build the substitution: [ [TR] , "s" , L ]
        // 'self.xre' (mut) and the arena entry (shared) are borrowed at once:
        // destructure 'self' into disjoint field borrows.
        let XfstCompiler { xre, nets, .. } = self;
        xre.define_transducer("TempXfstTransducerName", &nets[top.0]); // XRE
        let mut subst_regex = String::from("`[ [TempXfstTransducerName] , ");
        subst_regex.push_str(&format!("\"{}\" , {} ]", target, liststr));
        let substituted = self.xre.compile(&subst_regex); // XRE
        let substituted = substituted.map(|t| self.alloc_net(t));
        self.xre.undefine("TempXfstTransducerName"); // XRE

        if let Some(substituted) = substituted {
            // MAYBE_MINIMIZE(substituted)
            let cfg = self.engine_config;
            self.net_mut(substituted).optimize_with_config(&cfg)?;
            self.stack.push(substituted);
            self.print_transducer_info();
        } else {
            self.diag_error("fatal error in substitution");
            self.fail_flag = true;
        }
        self.prompt();
        Ok(self)
    }

    // Tokenize string \a s using \a c as separator.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.tokenize-string-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.tokenize-string-fn]
    fn tokenize_string(s: &str, c: char) -> StringVector {
        let mut retval: StringVector = Vec::new();
        let bytes = s.as_bytes();
        let mut pos: usize = 0;
        for i in 0..bytes.len() {
            if bytes[i] == c as u8 {
                retval.push(Symbol::new(&s[pos..i]));
                pos = i + 1;
            }
        }
        retval.push(Symbol::new(&s[pos..]));
        retval
    }

    // Convert StringVector \a sv into StringPair; None when the vector has
    // neither one nor two elements (the C++ threw a const char* here).
    // [spec:hfst:def:xfst-compiler.hfst.xfst.symbol-vector-to-symbol-pair-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.symbol-vector-to-symbol-pair-fn]
    fn symbol_vector_to_symbol_pair(sv: &StringVector) -> Option<StringPair> {
        let mut sp: StringPair = (Symbol::default(), Symbol::default());
        if sv.len() == 2 {
            if sv[0] == "?" {
                sp.0 = Symbol::new_static("@_UNKNOWN_SYMBOL_@");
            } else if sv[0] == "0" {
                sp.0 = Symbol::new_static("@_EPSILON_SYMBOL_@");
            } else {
                sp.0 = sv[0].clone();
            }

            if sv[1] == "?" {
                sp.1 = Symbol::new_static("@_UNKNOWN_SYMBOL_@");
            } else if sv[1] == "0" {
                sp.1 = Symbol::new_static("@_EPSILON_SYMBOL_@");
            } else {
                sp.1 = sv[1].clone();
            }
        } else if sv.len() == 1 {
            if sv[0] == "?" {
                // special case "?"
                sp.0 = Symbol::new_static("@_IDENTITY_SYMBOL_@");
            } else if sv[0] == "0" {
                sp.0 = Symbol::new_static("@_EPSILON_SYMBOL_@");
            } else {
                sp.0 = sv[0].clone();
            }
            sp.1 = sp.0.clone();
        } else {
            return None;
        }
        Some(sp)
    }
}
