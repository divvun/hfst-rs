//! The xfst variables (set, show) and the compiler settings they sit beside.

use super::*;

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    /// Report a `set` of a variable this compiler records but never consults.
    /// Silence is only correct while the requested value happens to describe
    /// what the compiler does anyway; anything else is a request it cannot
    /// honour, and the user is entitled to hear that rather than see it echoed.
    fn warn_if_inert(&self, name: &str, text: &str) {
        let Some((_, honest)) = INERT_VARIABLES.iter().find(|(v, _)| *v == name) else {
            return;
        };
        if *honest == Some(text) {
            return;
        }
        self.diag_warning_with_notes(
            &format!("'{}' is recorded but never consulted by this build", name),
            &[match honest {
                Some(value) => format!("the behaviour is always '{}'", value),
                None => String::from("no value of it changes anything"),
            }],
        );
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-precision-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-precision-fn]
    // @brief Get the precision that is used when printing weights.
    pub(super) fn get_precision(&mut self) -> i32 {
        // std::istringstream iss(variables["precision"]); iss >> retval;
        let s = self.variables["precision"].clone();
        Self::parse_int(&s)
    }

    // @brief Set variable @c name = @c text
    pub fn set(&mut self, name: &str, text: &str) -> &mut Self {
        if !self.variables.contains_key(name) {
            if name == "compose-flag-as-special" {
                self.diag_warning(
                    "there is no compose-flag-as-special variable; setting flag-is-epsilon instead",
                );
                self.variables
                    .insert("flag-is-epsilon".to_string(), text.to_string());
                if self.verbose {
                    println!("variable flag-is-epsilon = {}", text);
                }
                self.prompt();
                return self;
            } else {
                self.diag_unknown_variable(name);
                self.prompt();
                return self;
            }
        }
        self.variables.insert(name.to_string(), text.to_string());
        if name == "hopcroft-min" {
            if text == "ON" {
                self.engine_config.minimization_algorithm =
                    crate::hfst_transducer::MinimizationAlgorithm::HOPCROFT;
            }
            if text == "OFF" {
                self.engine_config.minimization_algorithm =
                    crate::hfst_transducer::MinimizationAlgorithm::BRZOZOWSKI;
                self.diag_warning_with_notes(
                    "this build has no Brzozowski minimizer: minimization stays Hopcroft",
                    &[String::from(
                        "the minimal network is the same either way; only the algorithm that \
                         reaches it would differ",
                    )],
                );
            }
        }
        if name == "encode-weights" {
            if text == "ON" {
                self.engine_config.encode_weights = true;
                self.xre.set_encode_weights(true);
            }
            if text == "OFF" {
                self.engine_config.encode_weights = false;
                self.xre.set_encode_weights(false);
            }
        }
        if name == "harmonize-flags" && matches!(text, "ON" | "OFF") {
            self.harmonize_flags = text == "ON";
            self.xre.set_flag_harmonization(self.harmonize_flags);
        }
        if name == "xerox-composition" {
            // C++ toggled the 'hfst::xerox_composition' global shared with the
            // XRE compiler; keep the xre compiler's copy in sync.
            if text == "ON" {
                self.engine_config.xerox_composition = true;
                self.xre.set_xerox_composition(true);
            }
            if text == "OFF" {
                self.engine_config.xerox_composition = false;
                self.xre.set_xerox_composition(false);
            }
        }
        if name == "flag-is-epsilon" {
            // Same global-sharing situation as xerox-composition above.
            if text == "ON" {
                self.engine_config.flag_is_epsilon_in_composition = true;
                self.xre.set_flag_is_epsilon(true);
            }
            if text == "OFF" {
                self.engine_config.flag_is_epsilon_in_composition = false;
                self.xre.set_flag_is_epsilon(false);
            }
        }
        if name == "minimal" {
            // C++ toggled the 'hfst::can_minimize' global that 'optimize' branches
            // on (minimize when set, determinize when clear). Here that choice
            // rides on the owned EngineConfig threaded into every 'optimize' call,
            // plus the XRE compiler's own copy so 'read regex' builds the same way.
            if text == "ON" {
                self.engine_config.minimization = true;
                self.xre.set_minimize_result(true);
            }
            if text == "OFF" {
                self.engine_config.minimization = false;
                self.xre.set_minimize_result(false);
            }
        }
        if name == "verbose" {
            // Upstream stored this and read it nowhere, so 'set verbose ON' was
            // inert there; the flag it obviously names is this compiler's own.
            self.verbose = text == "ON";
            self.xre.set_verbosity(self.verbose);
            self.lexc.set_verbosity(if self.verbose { 2 } else { 0 });
        }
        self.warn_if_inert(name, text);

        if self.verbose {
            println!("variable {} = {}", name, text);
        }

        self.prompt();
        self
    }

    // @brief Set variable @c name = @c number
    pub fn set_number(&mut self, name: &str, number: u32) -> &mut Self {
        if !self.variables.contains_key(name) {
            self.diag_unknown_variable(name);
            self.prompt();
            return self;
        }
        let num = format!("{}", number);
        self.variables.insert(name.to_string(), num);
        self.prompt();
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-fn]
    // @brief Get variable \a name.
    pub fn get(&mut self, name: &str) -> String {
        if !self.variables.contains_key(name) {
            return String::new();
        }
        self.variables[name].clone()
    }

    // @brief Show named variable
    pub fn show(&mut self, name: &str) -> &mut Self {
        if !self.variables.contains_key(name) {
            self.diag_unknown_variable(name);
            self.prompt();
            return self;
        }
        println!("variable {} = {}", name, self.variables[name]);
        self.prompt();
        self
    }

    // @brief Show all variables
    pub fn show_all(&mut self) -> &mut Self {
        let vars: Vec<(String, String)> = self
            .variables
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (first, second) in vars.iter() {
            if first == "copyright-owner" {
                println!("{:>20}: {}", first, second);
            } else {
                let explanation = variable_explanations_get(first);
                println!("{:>20}: {:>6}: {}", first, second, explanation);
            }
        }
        self.prompt();
        self
    }

    // @brief Define whether readline library is used to read input in apply up etc.
    pub fn set_readline(&mut self, readline: bool) -> &mut Self {
        self.use_readline = readline;
        self
    }

    // @brief Define whether input is read from stdin in apply up etc.
    pub fn set_read_interactive_text_from_stdin(&mut self, value: bool) -> &mut Self {
        self.read_interactive_text_from_stdin = value;
        self
    }

    // @brief Define whether output is printed directly to windows console.
    pub fn set_output_to_console(&mut self, value: bool) -> &mut Self {
        self.output_to_console = value;
        // hfst::print_output_to_console(output_to_console);
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-readline-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-readline-fn]
    // @brief Whether readline is used to read input in apply up etc.
    pub fn get_readline(&mut self) -> bool {
        self.use_readline
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-read-interactive-text-from-stdin-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-read-interactive-text-from-stdin-fn]
    // @brief Whether stdin is used to read input in apply up etc.
    pub fn get_read_interactive_text_from_stdin(&mut self) -> bool {
        self.read_interactive_text_from_stdin
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-output-to-console-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-output-to-console-fn]
    // @brief Whether output is printed directly to windows console.
    pub fn get_output_to_console(&mut self) -> bool {
        self.output_to_console
    }

    // @brief Define wheter prompts and XFST outputs are printed.
    pub fn set_verbosity(&mut self, verbosity: bool) -> &mut Self {
        self.verbose = verbosity;
        self.xre.set_verbosity(verbosity);
        self.lexc.set_verbosity(if self.verbose { 2 } else { 0 });
        self
    }

    // @brief Define wheter prompts are printed.
    pub fn set_prompt_verbosity(&mut self, verbosity: bool) -> &mut Self {
        self.verbose_prompt = verbosity;
        self
    }

    // @brief Allow read and write operations only in current directory, do not allow system calls.
    pub fn set_restricted_mode(&mut self, value: bool) -> &mut Self {
        self.restricted_mode = value;
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-restricted-mode-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-restricted-mode-fn]
    // @brief Whether restricted mode is on.
    pub fn get_restricted_mode(&self) -> bool {
        self.restricted_mode
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.quit-requested-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.quit-requested-fn]
    // @brief Whether it has been requested to quit the program.
    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-fail-flag-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-fail-flag-fn]
    // For xfst parser.
    pub fn get_fail_flag(&self) -> bool {
        self.fail_flag
    }
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.string-to-size-t-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.string-to-size-t-fn]
pub(super) fn parse_size(str: &str) -> usize {
    // Mirror 'std::istringstream iss(str); size_t size; iss >> size;':
    // read the leading integer, defaulting to 0 if none is present.
    let trimmed = str.trim_start();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<usize>().unwrap_or(0)
}

/// Variables this compiler records but never consults, each paired with the
/// value that does describe what it actually does (`None` where no value does).
/// Upstream carried these too and stayed silent about most of them; `set` warns
/// instead, because an accepted request that changes nothing is worse than a
/// refused one.
const INERT_VARIABLES: &[(&str, Option<&str>)] = &[
    ("char-encoding", Some("UTF-8")),
    ("copyright-owner", None),
    ("directory", Some("OFF")),
    ("quote-special", Some("OFF")),
    ("random-seed", None),
    ("recode-cp1252", Some("NEVER")),
    ("recursive-define", Some("OFF")),
    ("sort-arcs", None),
    ("use-timer", Some("OFF")),
];

// A side table mirroring the C++ file-static 'variable_explanations' map,
// consulted by show_all. Populated lazily on first use.
fn variable_explanations_get(key: &str) -> String {
    let explanations: &[(&str, &str)] = &[
        (
            "assert",
            "quit the application if test result is 0 and quit-on-fail is ON",
        ),
        (
            "att-epsilon",
            "epsilon symbol used when reading from att files",
        ),
        ("char-encoding", "character encoding used, always UTF-8"),
        ("copyright-owner", ""),
        ("directory", "<NOT IMPLEMENTED>"),
        ("encode-weights", "encode weights when minimizing"),
        (
            "flag-is-epsilon",
            "treat flag diacritics as epsilons in composition",
        ),
        (
            "harmonize-flags",
            "harmonize flag diacritics before composition",
        ),
        (
            "hopcroft-min",
            "use hopcroft's minimization algorithm, the only one built in",
        ),
        (
            "lexc-minimize-flags",
            "if 'lexc-with-flags' == ON, minimize number of flags",
        ),
        (
            "lexc-rename-flags",
            "if 'lexc-minimize-flags' == ON, rename flags",
        ),
        (
            "lexc-with-flags",
            "use flags to hyperminimize result from lexc files",
        ),
        ("maximum-weight", "maximum weight of paths printed in apply"),
        ("minimal", "minimize networks after operations"),
        (
            "name-nets",
            "stores the name of the network when using 'define'",
        ),
        ("obey-flags", "obey flag diacritic constraints"),
        ("precision", "todo: precision to use when printing weights"),
        ("print-foma-sigma", "print identities as '@'"),
        ("print-pairs", "show both sides (upper and lower) of labels"),
        ("print-sigma", "show sigma when printing a network"),
        (
            "print-space",
            "insert a space between symbols when printing words",
        ),
        (
            "print-weight",
            "show weights when printing words or networks",
        ),
        (
            "quit-on-fail",
            "quit the application if a command cannot be executed",
        ),
        (
            "quote-special",
            "<NOT IMPLEMENTED> enclose special characters in double quotes",
        ),
        ("random-seed", "<NOT IMPLEMENTED>"),
        ("recode-cp1252", "<NOT SUPPORTED>"),
        ("recursive-define", "<NOT IMPLEMENTED>"),
        (
            "retokenize",
            "retokenize regular expressions in 'compile-replace'",
        ),
        ("show-flags", "show flag diacritics when printing"),
        ("sort-arcs", "<NOT IMPLEMENTED>"),
        ("use-timer", "<NOT IMPLEMENTED>"),
        ("verbose", "print more information"),
        (
            "xerox-composition",
            "treat flag diacritics as ordinary symbols in composition",
        ),
    ];
    for (k, v) in explanations.iter() {
        if *k == key {
            return v.to_string();
        }
    }
    String::new()
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.initialize-variable-explanations-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.initialize-variable-explanations-fn]
pub(super) fn initialize_variable_explanations() {
    // The C++ free function populates a file-static variable_explanations map;
    // the port resolves explanations lazily via variable_explanations_get, so
    // there is nothing to initialize here.
}
