//! compile-replace: expanding the regexps embedded between ^[ and ^] on one
//! side of the top network.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // internal function
    pub fn compile_replace_net(&mut self, level: Level) -> crate::error::Result<&mut Self> {
        assert!(level != Level::BOTH_LEVELS);

        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let mut tmp_cp = HfstTransducer::new_copy(self.net(tmp))?;

        if level == Level::UPPER_LEVEL {
            tmp_cp.input_project()?;
        } else {
            // LOWER_LEVEL
            tmp_cp.output_project()?;
        }

        if Self::is_well_formed_for_compile_replace(&tmp_cp, &mut self.xre)? {
            if self.verbose {
                debug!("Network is well-formed.");
            }
        } else {
            if self.verbose {
                debug!("Network is not well-formed.");
            }
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }

        let level_is_upper = level == Level::UPPER_LEVEL;
        let level_not_upper = level != Level::UPPER_LEVEL;
        let retokenize_on = self.variables["retokenize"] == "ON";
        let cfg = self.engine_config;

        let mut fsm = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(self.net(tmp))?;
        let mut early_return = false;
        // The C++ wrapped this block in try/catch (const char*) and demoted a
        // malformed compile-replace regexp to a diagnostic.
        match fsm.find_replacements(level_is_upper) {
            Err(e) => {
                self.diag_error(&format!(
                    "compile-replace failed: {}",
                    e.message.unwrap_or_default()
                ));
            }
            Ok(replacement_map) => {
                'outer: for (start_state, replacements) in replacement_map.iter() {
                    for (end_state, sp) in replacements.iter() {
                        let regexp = Self::to_regexp(sp, level_is_upper, retokenize_on);
                        let literal_regexp = Self::to_literal_regexp(sp, level_not_upper);

                        let mut cross_product_regexp = String::from("[ ");
                        if level_is_upper {
                            cross_product_regexp.push_str(&regexp);
                            cross_product_regexp.push_str(" ] .x. [ ");
                            cross_product_regexp.push_str(&literal_regexp);
                            cross_product_regexp.push_str(" ]");
                        } else {
                            cross_product_regexp.push_str(&literal_regexp);
                            cross_product_regexp.push_str(" ] .x. [ ");
                            cross_product_regexp.push_str(&regexp);
                            cross_product_regexp.push_str(" ]");
                        }

                        let Some(mut replacement) = self.xre.compile(&cross_product_regexp) else {
                            self.diag_error(&format!(
                                "compile-replace could not compile the regex it built: {}",
                                cross_product_regexp
                            ));
                            early_return = true;
                            break 'outer;
                        };

                        let _ = replacement.optimize_with_config(&cfg);
                        let repl = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(
                            &replacement,
                        )?;
                        fsm.insert_transducer(*start_state, *end_state, &repl);
                    }
                }
            }
        }

        if early_return {
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }

        let result: NetId = self.alloc_net(HfstTransducer::new_from_basic(&fsm)?);

        // filter out regexps
        let mut cr = Self::contains_regexp_markers_on_one_side(&mut self.xre, level_is_upper);
        cr.optimize_with_config(&cfg)?;

        self.net_mut(result)
            .subtract(&cr, true)?
            .optimize_with_config(&cfg)?;
        self.net_mut(result).substitute_string(
            "@EPSILON_MARKER@",
            "@_EPSILON_SYMBOL_@",
            true,
            true,
        )?;
        self.stack.pop();
        self.stack.push(result);

        self.prompt();
        Ok(self)
    }

    // @brief Compile-replace lower
    pub fn compile_replace_lower_net(&mut self) -> crate::error::Result<&mut Self> {
        self.compile_replace_net(Level::LOWER_LEVEL)
    }

    // @brief Compile-replace upper
    pub fn compile_replace_upper_net(&mut self) -> crate::error::Result<&mut Self> {
        self.compile_replace_net(Level::UPPER_LEVEL)
    }

    // Returns an automaton that contains one ore more "^[" "^]" expressions.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.contains-regexps-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.contains-regexps-fn]
    fn contains_regexps(xre: &mut XreCompiler<B>) -> HfstTransducer<B> {
        let not_bracket_star = xre
            .compile("[? - \"^[\" - \"^]\"]* ;")
            .expect("constant XRE literal compiles"); // XRE
        xre.define_transducer("TempNotBracketStar", &not_bracket_star); // XRE
        // all paths that contain one or more well-formed ^[ ^] expressions
        let well_formed = xre
            .compile(
                "TempNotBracketStar \"^[\" TempNotBracketStar  [ \"^]\" TempNotBracketStar \"^[\"  TempNotBracketStar ]*  \"^]\" TempNotBracketStar ;",
            )
            .expect("constant XRE literal compiles");
        xre.undefine("TempNotBracketStar");
        well_formed
    }

    // XRE
    // [spec:hfst:def:xfst-compiler.hfst.xfst.contains-regexp-markers-on-one-side-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.contains-regexp-markers-on-one-side-fn]
    fn contains_regexp_markers_on_one_side(
        xre: &mut XreCompiler<B>,
        input_side: bool,
    ) -> HfstTransducer<B> {
        if input_side {
            xre.compile("[?:?|0:?|?:0]* [\"^[\":? | \"^]\":? | \"^[\":0 | \"^]\":0] [?:?|0:?|?:0]*")
        } else {
            // output side
            xre.compile("[?:?|0:?|?:0]* [?:\"^[\" | ?:\"^]\" | 0:\"^[\" | 0:\"^]\"] [?:?|0:?|?:0]*")
        }
        .expect("constant XRE literal compiles")
    }

    // @pre \a t must be an automaton  XRE
    // [spec:hfst:def:xfst-compiler.hfst.xfst.is-well-formed-for-compile-replace-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.is-well-formed-for-compile-replace-fn]
    fn is_well_formed_for_compile_replace(
        t: &HfstTransducer<B>,
        xre: &mut XreCompiler<B>,
    ) -> crate::error::Result<bool> {
        let well_formed = Self::contains_regexps(xre);
        // subtract those paths from copy of t
        let mut tc = HfstTransducer::new_copy(t)?;
        tc.subtract(&well_formed, true)?;
        // all paths that contain one or more ^[ or ^]
        let brackets = xre
            .compile("$[ \"^[\" | \"^]\" ] ;")
            .expect("constant XRE literal compiles");

        // test if the result is empty
        tc.intersect(&brackets, true)?;
        let empty: HfstTransducer<B> = HfstTransducer::new();
        let value = empty.compare(&tc, false)?;
        Ok(value)
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.to-literal-regexp-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.to-literal-regexp-fn]
    fn to_literal_regexp(path: &[(Symbol, Symbol)], input_side: bool) -> String {
        let mut pathstr = String::from("[");
        for it in path.iter() {
            let symbol = if input_side { &it.0 } else { &it.1 };
            if symbol.as_str() != crate::hfst_symbol_defs::internal_epsilon {
                pathstr.push('"');
                pathstr.push_str(symbol);
                pathstr.push_str("\" ");
            }
        }
        pathstr.push(']');
        if pathstr == "[]" {
            pathstr = String::from("[0]");
        }
        pathstr
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.to-regexp-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.to-regexp-fn]
    fn to_regexp(path: &[(Symbol, Symbol)], input_side: bool, retokenize: bool) -> String {
        let mut pathstr = String::from("[");
        for it in path.iter() {
            let symbol = if input_side { &it.0 } else { &it.1 };
            // ignore "^[" and "^]"
            if symbol.as_str() != "^]" && symbol.as_str() != "^[" {
                if symbol.as_str() != crate::hfst_symbol_defs::internal_epsilon {
                    pathstr.push_str(symbol);
                    if !retokenize {
                        pathstr.push(' ');
                    }
                }
            } else {
                // For better alignment
                pathstr.push_str("\"@EPSILON_MARKER@\"");
                if !retokenize {
                    pathstr.push(' ');
                }
            }
        }
        pathstr.push(']');
        if pathstr == "[]" {
            pathstr = String::from("[0]");
        }
        pathstr
    }
}
