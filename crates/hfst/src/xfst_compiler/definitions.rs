//! Definitions: networks, regex macro functions, aliases and lists.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // @brief Define alias for command sequence
    pub fn define_alias(&mut self, name: &str, commands: &str) -> &mut Self {
        self.aliases.insert(Symbol::new(name), commands.to_string());
        self.prompt();
        self
    }

    // @brief Define list by range
    // @todo lists are not supported by HFST
    // @todo Unicode ranges are not supported
    pub fn define_list_by_range(&mut self, name: &str, start: &str, end: &str) -> &mut Self {
        if (start.len() > 1) || (end.len() > 1) {
            self.diag_warning(&format!("unsupported unicode range {}-{}", start, end));
        }
        let mut l: BTreeSet<Symbol> = BTreeSet::new();
        let start_c = start.as_bytes().first().copied().unwrap_or(0);
        let end_c = end.as_bytes().first().copied().unwrap_or(0);
        let mut c = start_c;
        while c < end_c {
            let s = (c as char).to_string();
            l.insert(Symbol::from(s));
            c += 1;
        }
        self.lists.insert(Symbol::new(name), l);
        self
    }

    // @brief Define list by labels
    // @todo lists are not supportedd by HFST
    pub fn define_list(&mut self, name: &str, list: &str) -> &mut Self {
        if self.definitions.contains_key(name) {
            self.diag_error_with_notes(
                &format!("'{}' is already defined as a transducer", name),
                &[format!(
                    "a name cannot be both; 'undefine {}' first to redefine it as a list",
                    name
                )],
            );
            // MAYBE_QUIT
            if self.variables["quit-on-fail"] == "ON" {
                self.fail_flag = true;
            }
            self.prompt();
            return self;
        }
        let mut l: BTreeSet<Symbol> = BTreeSet::new();
        for token in list.split(' ') {
            if token.is_empty() {
                continue;
            }
            l.insert(Symbol::new(token));
        }
        self.lists.insert(Symbol::new(name), l.clone());
        self.xre.define_list(name, &l); // XRE
        self.prompt();
        self
    }

    // @brief Define regex macro
    pub fn define_xre(&mut self, name: &str, xre: &str) -> &mut Self {
        // When calling this function, the regex 'indata' should already have
        // been compiled into a transducer which should have been stored to
        // the variable latest_regex_compiled.

        if self.lists.contains_key(name) {
            self.diag_error_with_notes(
                &format!("'{}' is already defined as a list", name),
                &[format!(
                    "a name cannot be both; 'unlist {}' first to redefine it as a transducer",
                    name
                )],
            );
            // MAYBE_QUIT
            if self.variables["quit-on-fail"] == "ON" {
                self.fail_flag = true;
            }
            self.prompt();
            return self;
        }

        if self.latest_regex_compiled.is_some() {
            let compiled = self.xre.compile(xre);
            let compiled = compiled.map(|t| self.alloc_net(t));
            match compiled {
                Some(compiled) => {
                    self.define_transducer(name, compiled);
                    self.original_definitions
                        .insert(Symbol::new(name), xre.to_string());
                }
                None => {
                    self.diag_error(&format!(
                        "could not define '{}': its regex did not compile",
                        name
                    ));
                    self.xfst_fail();
                }
            }
        } else {
            self.diag_error(&format!(
                "could not define '{}': no regex was compiled for it",
                name
            ));
            self.xfst_fail();
        }
        self.prompt();
        self
    }

    // @brief Define regex macro
    pub fn define(&mut self, name: &str) -> &mut Self {
        // GET_TOP(top)
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        self.stack.pop();
        self.define_transducer(name, top);

        self.original_definitions
            .insert(Symbol::new(name), "<net taken from stack>".to_string());
        self.prompt();
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.define-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.define-fn]
    // @brief Define transducer
    pub fn define_transducer(&mut self, name: &str, transducer: NetId) {
        let was_defined = self.xre.is_definition(name);
        // 'self.xre' (mut) and the arena entry (shared) are borrowed at once:
        // destructure 'self' into disjoint field borrows.
        let XfstCompiler { xre, nets, .. } = self;
        xre.define_transducer(name, &nets[transducer.0]);
        if self.variables["name-nets"] == "ON" {
            self.net_mut(transducer).set_name(name);
        }
        // the net stays in the arena; the map entry is simply overwritten.
        self.definitions.remove(name);
        self.definitions.insert(Symbol::new(name), transducer);

        if self.verbose {
            if was_defined {
                print!("Redefined");
            } else {
                print!("Defined");
            }
            println!(" '{}'", name);
        }
    }

    // @brief Define regex macro function
    // @todo Regex parser does not support macro functions
    pub fn define_function(&mut self, prototype: &str, xre: &str) -> &mut Self {
        let Some(name) = Self::extract_function_name(prototype) else {
            self.diag_error(&format!(
                "could not read a function name out of the prototype '{}'",
                prototype
            ));
            self.xfst_fail();
            self.prompt();
            return self;
        };

        let Some(arguments) = Self::extract_function_arguments(prototype) else {
            self.diag_error(&format!(
                "could not read the argument list out of the prototype '{}'",
                prototype
            ));
            self.xfst_fail();
            self.prompt();
            return self;
        };

        let xre_converted =
            Self::convert_argument_symbols(&arguments, xre, &name, &mut self.xre, false);
        if xre_converted.is_empty() {
            self.diag_error(&format!("could not parse the body of function '{}'", name));
            self.xfst_fail();
            self.prompt();
            return self;
        }

        let was_defined = self.xre.is_function_definition(&name);

        if !self.xre.define_function(
            &name,
            u32::try_from(arguments.len()).expect("value out of u32 range"),
            &xre_converted,
        ) {
            // XRE
            self.diag_error(&format!("could not define function '{}'", name));
            self.xfst_fail();
            self.prompt();
            return self;
        }

        if self.verbose {
            if was_defined {
                print!("Redefined");
            } else {
                print!("Defined");
            }
            println!(" function '{}@{})", name, arguments.len() as i32);
        }

        self.function_arguments.insert(
            Symbol::from(name.clone()),
            u32::try_from(arguments.len()).expect("value out of u32 range"),
        );
        let fdef = Self::convert_argument_symbols(&arguments, xre, "", &mut self.xre, true);
        self.function_definitions
            .insert(Symbol::from(name.clone()), fdef);
        self.original_function_definitions
            .insert(Symbol::new(prototype), xre.to_string());

        self.prompt();
        self
    }

    // @brief Remove definition
    pub fn undefine(&mut self, name_list: &str) -> &mut Self {
        for name in name_list.split(' ') {
            if name.is_empty() {
                continue;
            }
            if self.definitions.remove(name).is_some() {
                self.xre.undefine(name); // XRE
            }
        }
        self.prompt();
        self
    }

    // @brief Remove list
    // @todo HFST does not support lists
    pub fn unlist(&mut self, name: &str) -> &mut Self {
        if self.lists.contains_key(name) {
            self.lists.remove(name);
        }
        self.prompt();
        self
    }

    // Extract the function name (up to and including the '(') from 'prototype'.
    // 'prototype' must be of format "functionname(arg1, arg2, ... argN)"
    // [spec:hfst:def:xfst-compiler.hfst.xfst.extract-function-name-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-function-name-fn]
    fn extract_function_name(prototype: &str) -> Option<String> {
        let mut name = String::new();
        for ch in prototype.chars() {
            if ch == '(' {
                return Some(name);
            }
            name.push(ch);
        }
        None // no starting parenthesis found
    }

    // Extract the names of function arguments from 'prototype'.
    // 'prototype' must be of format "functionname(arg1, arg2, ... argN)"
    // [spec:hfst:def:xfst-compiler.hfst.xfst.extract-function-arguments-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-function-arguments-fn]
    fn extract_function_arguments(prototype: &str) -> Option<Vec<String>> {
        let p: Vec<char> = prototype.chars().collect();
        let at = |i: usize| -> char { p.get(i).copied().unwrap_or('\0') };

        let mut args: Vec<String> = Vec::new();

        // skip the function name
        let mut i: usize = 0;
        while at(i) != '(' {
            if at(i) == '\0' {
                return None; // function name ended too early
            }
            i += 1;
        }
        i += 1; // skip the "(" in function name

        // start scanning the argument list "arg1, arg2, ... argN )"
        let mut arg = String::new();
        while at(i) != ')' {
            if at(i) == '\0' {
                // no closing parenthesis found
                return None;
            } else if at(i) == ' ' {
                // skip whitespace
            } else if at(i) == ',' {
                // end of argument
                args.push(arg.clone());
                arg = String::new();
            } else {
                arg.push(at(i));
            }
            i += 1;
        }
        // last argument
        args.push(arg);

        Some(args)
    }

    /// Whether `c` can continue an xre NAMETOKEN. C++'s NAMECHAR is any printable
    /// ASCII except the operator/bracket set, plus non-ASCII; used to tell a
    /// standalone parameter from a substring of a longer name.
    fn is_name_char(c: char) -> bool {
        if (c as u32) > 0x7e {
            return true;
        }
        if (c as u32) < 0x21 {
            return false; // space and control characters
        }
        !matches!(
            c,
            '<' | '>'
                | '%'
                | '('
                | ')'
                | '['
                | ']'
                | '!'
                | ';'
                | ':'
                | '"'
                | ','
                | '|'
                | '&'
                | '*'
                | '+'
                | '-'
                | '~'
                | '$'
                | '/'
                | '\\'
                | '.'
                | '{'
                | '}'
                | '^'
                | '='
                | '?'
        )
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.convert-argument-symbols-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.convert-argument-symbols-fn]
    fn convert_argument_symbols(
        arguments: &[String],
        xre: &str,
        function_name: &str,
        _xre_compiler: &mut XreCompiler<B>,
        user_friendly_argument_names: bool,
    ) -> String {
        // Rewrite each parameter name in the body to the placeholder symbol
        // that eval_function_call binds the actual argument to.
        //
        // C++ located the parameters via positions its flex/bison scanner
        // recorded while compiling; nfst replaces that lexer, so the position
        // set is always empty and the port left the body untouched — every
        // argument then failed to substitute, and a compound argument silently
        // compiled as a bare symbol.
        //
        // Matching whole tokens instead: a parameter is a NAMETOKEN, so an
        // occurrence counts only when neither neighbour could continue it.
        let mut retval: String = xre.to_string();

        for (arg_index, argument) in arguments.iter().enumerate() {
            if argument.is_empty() {
                continue;
            }
            let replacement: String = if user_friendly_argument_names {
                format!("ARGUMENT{}", arg_index + 1)
            } else {
                format!("\"@{}{}@\"", function_name, arg_index + 1)
            };

            let mut out = String::with_capacity(retval.len());
            let mut rest: &str = &retval;
            while let Some(hit) = rest.find(argument.as_str()) {
                let before = &rest[..hit];
                let after = &rest[hit + argument.len()..];
                let joined_left = before.chars().next_back().is_some_and(Self::is_name_char);
                let joined_right = after.chars().next().is_some_and(Self::is_name_char);
                out.push_str(before);
                if joined_left || joined_right {
                    out.push_str(argument); // part of a longer name, leave it
                } else {
                    out.push_str(&replacement);
                }
                rest = after;
            }
            out.push_str(rest);
            retval = out;
        }

        retval
    }
}
