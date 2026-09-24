//! Definition expansion, the label-to-transducer builders, merge, and the
//! function-argument helpers.

use super::*;

impl<B: AlgebraBackend> XreCompiler<B> {
    // ----------------------------------------------------------------
    // Definitions
    // ----------------------------------------------------------------

    // xre_utils.cc:837 'HfstTransducer* expand_definition(const char* symbol)'
    pub(super) fn expand_definition_sym(
        &self,
        symbol: &str,
    ) -> crate::error::Result<HfstTransducer<B>> {
        if self.expand_definitions {
            for (k, v) in self.definitions.iter() {
                if k.as_str() == symbol {
                    return Ok(v.clone());
                }
            }
            // PORT DIVERGENCE: upstream reads a bare token as a symbol, so a
            // function name compiles to a literal acceptor for its own name.
            // That is always a missing argument list or a typo.
            if self.function_definitions.contains_key(symbol) {
                crate::bail!(
                    Hfst,
                    format!(
                        "'{0}' is a function, not a network: apply it with arguments as '{0}(...)'. \
                         A bare name never resolves to a function — upstream compiles it as the \
                         literal symbol '{0}' instead, silently.",
                        symbol
                    )
                );
            }
        }
        HfstTransducer::new_symbol_pair(symbol, symbol)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.expand-definition-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.expand-definition-fn]
    // xre_utils.cc:857 'HfstTransducer* expand_definition(HfstTransducer*, const char*)'
    fn expand_definition_tr(
        &self,
        tr: &mut HfstTransducer<B>,
        symbol: &str,
    ) -> crate::error::Result<()> {
        if self.expand_definitions {
            for (k, v) in self.definitions.iter() {
                if k.as_str() == symbol {
                    let alpha = v.get_alphabet()?;
                    let mut v_clone = v.clone();
                    tr.substitute_pair_with_transducer(
                        &(Symbol::new(symbol), Symbol::new(symbol)),
                        &mut v_clone,
                        false, // do not harmonize
                    )?;
                    if !alpha.contains(symbol) {
                        tr.remove_from_alphabet_string(symbol)?;
                    }
                    break;
                }
            }
        }
        Ok(())
    }

    // ----------------------------------------------------------------
    // Label -> transducer builders
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
    // xre_utils.cc:959
    pub(super) fn xfst_label_to_transducer(
        &self,
        input: &str,
        output: &str,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let input_is_definition = self.definitions.contains_key(input);
        let output_is_definition = self.definitions.contains_key(output);
        let input_is_unknown = input == crate::hfst_symbol_defs::internal_unknown;
        let output_is_unknown = output == crate::hfst_symbol_defs::internal_unknown;

        // definitions -> use cross-product
        if input_is_definition || output_is_definition {
            let mut retval;
            let tmp;
            if input_is_unknown {
                retval = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
                tmp = self.expand_definition_sym(output)?;
            } else if output_is_unknown {
                tmp = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
                retval = self.expand_definition_sym(input)?;
            } else {
                retval = self.expand_definition_sym(input)?;
                tmp = self.expand_definition_sym(output)?;
            }
            retval.cross_product(&tmp, true)?;
            return Ok(retval);
        }

        // no definitions
        Ok(if input_is_unknown && output_is_unknown {
            let mut retval = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_unknown,
                crate::hfst_symbol_defs::internal_unknown,
            )?;
            let id = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_identity,
                crate::hfst_symbol_defs::internal_identity,
            )?;
            retval
                .disjunct(&id, true)?
                .minimize_with_config(&self.opt_cfg())?;
            retval
        } else if input_is_unknown {
            let mut retval =
                HfstTransducer::new_symbol_pair(crate::hfst_symbol_defs::internal_unknown, output)?;
            let output_tr = HfstTransducer::new_symbol_pair(output, output)?;
            retval
                .disjunct(&output_tr, true)?
                .minimize_with_config(&self.opt_cfg())?;
            retval
        } else if output_is_unknown {
            let mut retval =
                HfstTransducer::new_symbol_pair(input, crate::hfst_symbol_defs::internal_unknown)?;
            let input_tr = HfstTransducer::new_symbol_pair(input, input)?;
            retval
                .disjunct(&input_tr, true)?
                .minimize_with_config(&self.opt_cfg())?;
            retval
        } else {
            HfstTransducer::new_symbol_pair(input, output)?
        })
    }

    // [spec:hfst:def:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
    // xre_utils.cc:897
    pub(super) fn xfst_curly_label_to_transducer(
        &self,
        input: &str,
        output: &str,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut retval;

        if input == crate::hfst_symbol_defs::internal_unknown {
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let sv = tok.tokenize_one_level(output, false);
            let first_token = sv[0].clone();
            retval = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_unknown,
                &first_token,
            )?;
            for it in sv.iter() {
                let tmp = HfstTransducer::new_symbol_pair(it, &first_token)?;
                retval.disjunct(&tmp, false)?;
            }
            for it in sv.iter().skip(1) {
                let tmp =
                    HfstTransducer::new_symbol_pair(crate::hfst_symbol_defs::internal_epsilon, it)?;
                retval.concatenate(&tmp, false)?;
            }
        } else if output == crate::hfst_symbol_defs::internal_unknown {
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let sv = tok.tokenize_one_level(input, false);
            let first_token = sv[0].clone();
            retval = HfstTransducer::new_symbol_pair(
                &first_token,
                crate::hfst_symbol_defs::internal_unknown,
            )?;
            for it in sv.iter() {
                let tmp = HfstTransducer::new_symbol_pair(&first_token, it)?;
                retval.disjunct(&tmp, false)?;
            }
            for it in sv.iter().skip(1) {
                let tmp =
                    HfstTransducer::new_symbol_pair(it, crate::hfst_symbol_defs::internal_epsilon)?;
                retval.concatenate(&tmp, false)?;
            }
        } else {
            let mut tok = crate::hfst_tokenizer::HfstTokenizer::new();
            tok.add_multichar_symbol(crate::hfst_symbol_defs::internal_epsilon);
            retval = HfstTransducer::new_tokenized_pair(input, output, &tok)?;
        }

        retval.minimize_with_config(&self.opt_cfg())?; // it should be safe to minimize
        Ok(retval)
    }

    // ----------------------------------------------------------------
    // Merge
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.merge-first-to-second-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.merge-first-to-second-fn]
    // xre_utils.cc:1214. 'tr1' is optimized then merged into 'tr2' (returned).
    pub(super) fn merge_first_to_second(
        &self,
        tr1: &mut HfstTransducer<B>,
        mut tr2: HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // Merge operation creates an XreCompiler that needs this information
        // below; otherwise it would overwrite all of it.
        let args = XreConstructorArguments {
            definitions: self.definitions.clone(),
            function_definitions: self.function_definitions.clone(),
            function_arguments: self.function_arguments.clone(),
            list_definitions: self.list_definitions.clone(),
        };
        tr1.optimize_with_config(&self.opt_cfg())?;
        tr2.merge(tr1, &args)?;
        Ok(tr2)
    }

    // ----------------------------------------------------------------
    // Function-call helpers (definitions named "@<name><N>@", 1-based)
    // 'name' must include its trailing '(' as stored by 'define_function'.
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.is-valid-function-call-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.is-valid-function-call-fn]
    fn is_valid_function_call(&self, name: &str, args: &[HfstTransducer<B>]) -> bool {
        let name2xre = self.function_definitions.get(name);
        let name2args = self.function_arguments.get(name);

        if name2xre.is_none() || name2args.is_none() {
            self.diag_error(&format!("No such function defined: '{}'", name));
            return false;
        }

        let number_of_args = *name2args.expect("name2args is Some, checked above");

        if number_of_args as usize != args.len() {
            self.diag_error(&format!(
                "Wrong number of arguments: function '{}' expects {}, {} given",
                name,
                number_of_args as i32,
                args.len() as i32
            ));
            return false;
        }
        true
    }

    // [spec:hfst:def:xre-utils.hfst.xre.get-function-xre-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.get-function-xre-fn]
    fn get_function_xre(&self, name: &str) -> Option<String> {
        self.function_definitions.get(name).cloned()
    }

    // [spec:hfst:def:xre-utils.hfst.xre.define-function-args-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.define-function-args-fn]
    fn define_function_args(&mut self, name: &str, args: &[HfstTransducer<B>]) -> bool {
        if !self.is_valid_function_call(name, args) {
            return false;
        }
        for (i, it) in args.iter().enumerate() {
            let arg_number: u32 = i as u32 + 1;
            let function_arg = Symbol::from(format!("@{}{}@", name, arg_number));
            self.definitions.insert(function_arg, it.clone());
        }
        true
    }

    // [spec:hfst:def:xre-utils.hfst.xre.undefine-function-args-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.undefine-function-args-fn]
    fn undefine_function_args(&mut self, name: &str) {
        let n = match self.function_arguments.get(name) {
            Some(n) => *n,
            None => return,
        };
        for arg_number in 1..=n {
            let function_arg = format!("@{}{}@", name, arg_number);
            self.definitions.remove(function_arg.as_str());
        }
    }
}
