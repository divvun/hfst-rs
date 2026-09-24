//! The nfst-xfst command-dispatch driver, which replaces the bison action
//! dispatch.

use super::*;
use nfst_xfst::{
    ApplyKind, NetworkOp, ReadCmd, Redirect, RedirectKind, SubstituteCmd, XfstCommand,
};

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // @brief Dispatch a single XfstCommand parsed by nfst-xfst, calling the
    // corresponding ported command-handler method 1:1. Each arm mirrors the
    // bison action of the matching grammar production in xfst-parser.yy.
    pub(super) fn eval_command(&mut self, cmd: &XfstCommand) -> crate::error::Result<()> {
        match cmd {
            // ── regex / define ──────────────────────────────
            XfstCommand::Regex(xre) => {
                self.eval_regex(xre)?;
            }
            XfstCommand::Define { name, body } => {
                self.eval_define(name, body)?;
            }
            XfstCommand::DefineFunction { name, params, body } => {
                let prototype = format!("{}({})", name, params.join(", "));
                let xre = nfst_xre::pretty_print(body);
                self.define_function(&prototype, &xre);
            }
            XfstCommand::DefineAlias { name, body } => {
                self.define_alias(name, body);
            }
            XfstCommand::DefineList { name, members } => {
                self.eval_define_list(name, members);
            }
            XfstCommand::Undefine(names) => {
                self.undefine(&names.join(" "));
            }
            XfstCommand::Unlist(name) => {
                self.unlist(name);
            }

            // ── stack ───────────────────────────────────────
            XfstCommand::Clear => {
                self.clear();
            }
            XfstCommand::Pop => {
                self.pop();
            }
            XfstCommand::Push(name) => {
                self.eval_push(name)?;
            }
            XfstCommand::Turn => {
                self.turn();
            }
            XfstCommand::Rotate => {
                self.rotate();
            }
            XfstCommand::LoadStack(name) => {
                self.load_stack(name);
            }
            XfstCommand::LoadDefinitions(name) => {
                self.load_definitions(name);
            }

            // ── network ops ─────────────────────────────────
            XfstCommand::Network(op) => {
                self.eval_network_op(op)?;
            }

            // ── apply / lookup ──────────────────────────────
            XfstCommand::Apply(kind, input) => {
                self.eval_apply(kind, input)?;
            }
            XfstCommand::LookupOptimize => {
                self.lookup_optimize()?;
            }
            XfstCommand::RemoveOptimization => {
                self.remove_optimization()?;
            }

            // ── read / save ─────────────────────────────────
            XfstCommand::Read(rc) => {
                self.eval_read(rc)?;
            }
            XfstCommand::Save(sc) => {
                self.eval_save(sc)?;
            }

            // ── print ───────────────────────────────────────
            XfstCommand::Print(p) => {
                let mut out = std::io::stdout();
                self.eval_print(p, &mut out)?;
            }

            // ── test ────────────────────────────────────────
            XfstCommand::Test(kind) => {
                self.eval_test(*kind, false)?;
            }

            // ── variables / show ────────────────────────────
            XfstCommand::Set { var, value } => {
                self.eval_set(var, value);
            }
            XfstCommand::Show(opt) => {
                self.eval_show(opt);
            }
            XfstCommand::Echo(text) => {
                self.echo(text);
            }
            XfstCommand::System(command) => {
                self.system(command);
            }
            XfstCommand::Source(_path) => {
                // hxfsterror("source not implemented yywrap\n"); return EXIT_FAILURE;
                self.diag_error("'source' is not implemented; run the script with -F instead");
                self.fail_flag = true;
            }
            XfstCommand::Quit => {
                self.quit("bye");
            }

            // ── substitute ──────────────────────────────────
            XfstCommand::Substitute(sub) => {
                self.eval_substitute(sub)?;
            }

            // ── help / misc ─────────────────────────────────
            XfstCommand::Apropos(opt) => {
                let text = opt.as_deref().unwrap_or("");
                self.apropos(text);
            }
            XfstCommand::Describe(text) => {
                self.describe(text);
            }
            XfstCommand::Assert(inner) => {
                self.eval_assert(&inner.value)?;
            }
            XfstCommand::AddProps(content) => {
                self.add_props(content);
            }
            XfstCommand::EditProps => {
                // hxfsterror("NETWORK PROPERTY EDITOR unimplemented\n");
                // return EXIT_FAILURE;
                self.diag_error("the network property editor is not implemented");
                self.fail_flag = true;
            }
            XfstCommand::Hfst(data) => {
                self.hfst(data);
            }
            XfstCommand::For => {
                // 'for' has no standalone production in the bison grammar.
                self.prompt();
            }

            // ── i/o redirect wrapper ────────────────────────
            XfstCommand::Redirected { command, redirect } => {
                self.eval_redirected(command, redirect)?;
            }
        }
        Ok(())
    }

    // @brief The 'regex' command: compile the regex and push it.
    fn eval_regex(&mut self, xre: &nfst_xre::SpannedXre) -> crate::error::Result<()> {
        // compile_regex stored the freshly compiled regex into
        // latest_regex_compiled, then read_regex pushed a copy of it.
        if self.latest_regex_compiled.is_some() {
            self.latest_regex_compiled = None;
        }
        let compiled = self.compile_spanned_xre(xre)?;
        self.latest_regex_compiled = Some(compiled);
        self.read_regex("")?;
        Ok(())
    }

    // @brief The 'define' command, with or without a regex body.
    fn eval_define(&mut self, name: &str, body: &nfst_xre::SpannedXre) -> crate::error::Result<()> {
        // 'define NAME' / 'define NAME ;' with no regex body is the
        // C++ DEFINE_NAME production: it defines NAME as the network
        // popped from the top of the stack. nfst-xfst encodes the
        // missing body as Epsilon with an empty span, which is
        // distinguishable from an explicit epsilon regex ('define
        // NAME 0 ;', non-empty span).
        if body.span.is_empty() && matches!(body.value, nfst_xre::XreExpr::Epsilon) {
            self.define(name);
        } else {
            let tr = self.compile_spanned_xre(body)?;
            self.define_transducer(name, tr);
            // 'print defined' reports from original_definitions.
            self.original_definitions
                .insert(Symbol::new(name), nfst_xre::pretty_print(body).to_string());
            self.prompt();
        }
        Ok(())
    }

    // @brief The 'list' command.
    fn eval_define_list(&mut self, name: &str, members: &[String]) {
        // 'list NAME a-b' becomes a range; 'list NAME s1 s2 ...' a list.
        if members.len() == 1 && members[0].contains('-') {
            let m = &members[0];
            let idx = m.find('-').expect("contains '-', checked above");
            let start = &m[..idx];
            let end = &m[idx + 1..];
            self.define_list_by_range(name, start, end);
        } else {
            self.define_list(name, &members.join(" "));
        }
    }

    // @brief The 'push' command: a named definition, or every definition.
    fn eval_push(&mut self, name: &str) -> crate::error::Result<()> {
        if name.is_empty() {
            self.push_latest()?;
        } else {
            self.push(name)?;
        }
        Ok(())
    }

    // @brief Dispatch a parsed NetworkOp to the corresponding *_net method.
    fn eval_network_op(&mut self, op: &NetworkOp) -> crate::error::Result<()> {
        match op {
            NetworkOp::Compose => {
                self.compose_net()?;
            }
            NetworkOp::Concatenate => {
                self.concatenate_net()?;
            }
            NetworkOp::Intersect => {
                self.intersect_net()?;
            }
            NetworkOp::Union => {
                self.union_net()?;
            }
            NetworkOp::Minus => {
                self.minus_net()?;
            }
            NetworkOp::Crossproduct => {
                self.crossproduct_net()?;
            }
            NetworkOp::Ignore => {
                self.ignore_net()?;
            }
            NetworkOp::Invert => {
                self.invert_net()?;
            }
            NetworkOp::Reverse => {
                self.reverse_net()?;
            }
            NetworkOp::Determinize => {
                self.determinize_net()?;
            }
            NetworkOp::Minimize => {
                self.minimize_net()?;
            }
            NetworkOp::EpsilonRemove => {
                self.epsilon_remove_net()?;
            }
            NetworkOp::PruneNet => {
                self.prune_net()?;
            }
            NetworkOp::Negate => {
                self.negate_net()?;
            }
            NetworkOp::OnePlus => {
                self.one_plus_net()?;
            }
            NetworkOp::ZeroPlus => {
                self.zero_plus_net()?;
            }
            NetworkOp::Sort => {
                self.sort_net();
            }
            NetworkOp::Shuffle => {
                self.shuffle_net()?;
            }
            NetworkOp::Substring => {
                self.substring_net();
            }
            NetworkOp::Cleanup => {
                self.cleanup_net();
            }
            NetworkOp::Complete => {
                self.complete_net()?;
            }
            NetworkOp::LowerSide => {
                self.lower_side_net()?;
            }
            NetworkOp::UpperSide => {
                self.upper_side_net()?;
            }
            NetworkOp::Sigma => {
                self.sigma_net()?;
            }
            NetworkOp::LabelNet => {
                self.label_net()?;
            }
            NetworkOp::Inspect => {
                self.inspect_net()?;
            }
            NetworkOp::TwosidedFlags => {
                self.twosided_flags()?;
            }
            NetworkOp::EliminateAll => {
                self.eliminate_flags()?;
            }
            NetworkOp::CollectEpsilonLoops => {
                self.collect_epsilon_loops();
            }
            NetworkOp::CompactSigma => {
                self.compact_sigma()?;
            }
            NetworkOp::View => {
                self.view_net();
            }
            NetworkOp::ExtractAmbiguous | NetworkOp::ExtractUnambiguous | NetworkOp::Ambiguous => {
                // hxfsterror("unimplemetend ambiguous\n"); return EXIT_FAILURE;
                self.diag_error("the ambiguity commands are not implemented");
                self.fail_flag = true;
            }
            NetworkOp::CompileReplaceLower => {
                self.compile_replace_lower_net()?;
            }
            NetworkOp::CompileReplaceUpper => {
                self.compile_replace_upper_net()?;
            }
            NetworkOp::EliminateFlag(name) => {
                self.eliminate_flag(name);
            }
            NetworkOp::Name(name) => {
                self.name_net(name);
            }
        }
        Ok(())
    }

    // @brief The 'apply up/down/med' commands, reading stdin when no input
    // string was given.
    fn eval_apply(&mut self, kind: &ApplyKind, input: &Option<String>) -> crate::error::Result<()> {
        let s: String = match input {
            Some(s) => s.clone(),
            None => {
                // The bison grammar read from stdin here (apply_up(stdin)).
                use std::io::Read;
                let mut buf = String::new();
                let _ = std::io::stdin().read_to_string(&mut buf);
                buf
            }
        };
        match kind {
            ApplyKind::Up => {
                self.apply_up(&s)?;
            }
            ApplyKind::Down => {
                self.apply_down(&s)?;
            }
            ApplyKind::Med => {
                self.apply_med(&s);
            }
        }
        Ok(())
    }

    // @brief Dispatch a parsed ReadCmd to the corresponding read_* method.
    fn eval_read(&mut self, rc: &ReadCmd) -> crate::error::Result<()> {
        match rc {
            ReadCmd::Text(s) => {
                if s.contains('\n') {
                    self.read_text(s);
                } else {
                    self.read_text_from_file(s)?;
                }
            }
            ReadCmd::Spaced(s) => {
                if s.contains('\n') {
                    self.read_spaced(s);
                } else {
                    self.read_spaced_from_file(s)?;
                }
            }
            ReadCmd::Prolog(p) => {
                let s = std::fs::read_to_string(p).unwrap_or_default();
                self.read_prolog(&s);
            }
            ReadCmd::Props(p) => {
                let s = std::fs::read_to_string(p).unwrap_or_default();
                self.read_props(&s);
            }
            ReadCmd::Lexc(p) => {
                self.read_lexc_from_file(p)?;
            }
            ReadCmd::Att(p) => {
                self.read_att_from_file(p)?;
            }
        }
        Ok(())
    }

    // @brief The 'set' command: a numeric value or a textual one.
    fn eval_set(&mut self, var: &str, value: &str) {
        // int i = nametoken_to_number(value);
        // if (i != -1) set(var, i); else set(var, value);
        let trimmed = value.trim_start();
        let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
        match digits.parse::<u32>() {
            Ok(i) if !digits.is_empty() => {
                self.set_number(var, i);
            }
            _ => {
                self.set(var, value);
            }
        }
    }

    // @brief The 'show' command: one variable, or all of them.
    fn eval_show(&mut self, opt: &Option<String>) {
        match opt {
            Some(name) => {
                self.show(name);
            }
            None => {
                self.show_all();
            }
        }
    }

    // @brief Dispatch a parsed SubstituteCmd to the corresponding
    // substitute_* method.
    fn eval_substitute(&mut self, sub: &SubstituteCmd) -> crate::error::Result<()> {
        match sub {
            SubstituteCmd::Symbol { from, to, scope: _ } => {
                // substitute_symbol expects a quoted list: "s1" "s2" ...
                let list: String = from
                    .iter()
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join(" ");
                self.substitute_symbol(&list, to)?;
            }
            SubstituteCmd::Label { from, to, scope: _ } => {
                self.substitute_label(&from.join(" "), to)?;
            }
            SubstituteCmd::Named { def, label } => {
                self.substitute_named(def, label)?;
            }
        }
        Ok(())
    }

    // @brief The 'assert' command.
    fn eval_assert(&mut self, inner: &XfstCommand) -> crate::error::Result<()> {
        // The bison grammar only allows 'assert TEST' productions, each
        // of which calls test_X(true).
        if let XfstCommand::Test(kind) = inner {
            self.eval_test(*kind, true)?;
        } else {
            self.eval_command(inner)?;
        }
        Ok(())
    }

    // @brief Run a command under an i/o redirect: output to a file for
    // '>' and '>>', input from a file for '<'.
    fn eval_redirected(
        &mut self,
        command: &nfst_xfst::Spanned<XfstCommand>,
        redirect: &Redirect,
    ) -> crate::error::Result<()> {
        let inner = &command.value;
        match redirect.kind {
            RedirectKind::Out | RedirectKind::Append => {
                if self.check_filename(&redirect.path) {
                    let opened = match redirect.kind {
                        RedirectKind::Append => std::fs::OpenOptions::new()
                            .append(true)
                            .create(true)
                            .open(&redirect.path),
                        RedirectKind::In | RedirectKind::Out => {
                            std::fs::File::create(&redirect.path)
                        }
                    };
                    if let Ok(mut f) = opened {
                        match inner {
                            XfstCommand::Print(p) => {
                                self.eval_print(p, &mut f)?;
                            }
                            XfstCommand::Save(nfst_xfst::SaveCmd::Att(_)) => {
                                self.write_att(&mut f);
                            }
                            XfstCommand::Regex(_)
                            | XfstCommand::Define { .. }
                            | XfstCommand::DefineFunction { .. }
                            | XfstCommand::DefineAlias { .. }
                            | XfstCommand::DefineList { .. }
                            | XfstCommand::Undefine(_)
                            | XfstCommand::Unlist(_)
                            | XfstCommand::Clear
                            | XfstCommand::Pop
                            | XfstCommand::Push(_)
                            | XfstCommand::Turn
                            | XfstCommand::Rotate
                            | XfstCommand::LoadStack(_)
                            | XfstCommand::LoadDefinitions(_)
                            | XfstCommand::Network(_)
                            | XfstCommand::Apply(..)
                            | XfstCommand::LookupOptimize
                            | XfstCommand::RemoveOptimization
                            | XfstCommand::Read(_)
                            | XfstCommand::Save(_)
                            | XfstCommand::Test(_)
                            | XfstCommand::Set { .. }
                            | XfstCommand::Show(_)
                            | XfstCommand::Echo(_)
                            | XfstCommand::System(_)
                            | XfstCommand::Source(_)
                            | XfstCommand::Quit
                            | XfstCommand::Substitute(_)
                            | XfstCommand::Apropos(_)
                            | XfstCommand::Describe(_)
                            | XfstCommand::Assert(_)
                            | XfstCommand::AddProps(_)
                            | XfstCommand::EditProps
                            | XfstCommand::Hfst(_)
                            | XfstCommand::For
                            | XfstCommand::Redirected { .. } => {
                                self.eval_command(inner)?;
                            }
                        }
                    }
                }
            }
            RedirectKind::In => {
                let path = &redirect.path;
                match inner {
                    XfstCommand::Apply(kind, _) => {
                        let s = std::fs::read_to_string(path).unwrap_or_default();
                        match kind {
                            ApplyKind::Up => {
                                self.apply_up(&s)?;
                            }
                            ApplyKind::Down => {
                                self.apply_down(&s)?;
                            }
                            ApplyKind::Med => {
                                self.apply_med(&s);
                            }
                        }
                    }
                    XfstCommand::AddProps(_) => {
                        let s = std::fs::read_to_string(path).unwrap_or_default();
                        self.add_props(&s);
                    }
                    XfstCommand::Read(ReadCmd::Props(_)) => {
                        let s = std::fs::read_to_string(path).unwrap_or_default();
                        self.read_props(&s);
                    }
                    XfstCommand::LoadStack(_) => {
                        self.load_stack(path);
                    }
                    XfstCommand::LoadDefinitions(_) => {
                        self.load_definitions(path);
                    }
                    XfstCommand::Regex(_)
                    | XfstCommand::Define { .. }
                    | XfstCommand::DefineFunction { .. }
                    | XfstCommand::DefineAlias { .. }
                    | XfstCommand::DefineList { .. }
                    | XfstCommand::Undefine(_)
                    | XfstCommand::Unlist(_)
                    | XfstCommand::Clear
                    | XfstCommand::Pop
                    | XfstCommand::Push(_)
                    | XfstCommand::Turn
                    | XfstCommand::Rotate
                    | XfstCommand::Network(_)
                    | XfstCommand::LookupOptimize
                    | XfstCommand::RemoveOptimization
                    | XfstCommand::Read(_)
                    | XfstCommand::Save(_)
                    | XfstCommand::Print(_)
                    | XfstCommand::Test(_)
                    | XfstCommand::Set { .. }
                    | XfstCommand::Show(_)
                    | XfstCommand::Echo(_)
                    | XfstCommand::System(_)
                    | XfstCommand::Source(_)
                    | XfstCommand::Quit
                    | XfstCommand::Substitute(_)
                    | XfstCommand::Apropos(_)
                    | XfstCommand::Describe(_)
                    | XfstCommand::Assert(_)
                    | XfstCommand::EditProps
                    | XfstCommand::Hfst(_)
                    | XfstCommand::For
                    | XfstCommand::Redirected { .. } => {
                        self.eval_command(inner)?;
                    }
                }
            }
        }
        Ok(())
    }

    // @brief Compile a fully-parsed SpannedXre into a transducer by walking
    // it with self.xre. Mirrors the regex-compile path the bison actions used.
    // The XreCompiler::compile string entry point parses then walks the tree;
    // here the tree is already parsed, so we walk it directly and optimize,
    // returning a shared handle just like xre.compile.
    fn compile_spanned_xre(&mut self, xre: &nfst_xre::SpannedXre) -> crate::error::Result<NetId> {
        // An embedded body is parsed by nfst-xfst as a standalone string, so
        // only its root node carries a script span and every inner node counts
        // from the body's first byte. Those spans would be read against
        // whatever source a previous 'xre.compile' left behind, putting a caret
        // under unrelated text; clearing it makes the regex compiler fall back
        // to plain messages here. Body syntax errors are anchored properly a
        // layer up, where the body is re-parsed at its script offset.
        self.xre.source = String::new();
        let t = self.xre.eval_finalized(xre)?;
        Ok(self.alloc_net(t))
    }

    // @brief Dispatch a parsed PrintCmd to the corresponding print* method,
    // writing to \a oss (stdout for plain commands, a file for redirected ones).
    fn eval_print(
        &mut self,
        p: &nfst_xfst::PrintCmd,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<()> {
        use nfst_xfst::PrintCmd as P;
        match p {
            P::Net => {
                self.print_net(oss)?;
            }
            P::Stack => {
                self.print_stack(oss);
            }
            P::Sigma => {
                self.print_sigma(oss, true)?;
            }
            P::SigmaCount => {
                self.print_sigma_count(oss);
            }
            P::SigmaWordCount => {
                self.print_sigma_word_count(oss);
            }
            P::Size => {
                self.print_size(oss);
            }
            P::LongestString => {
                self.print_longest_string(oss)?;
            }
            P::LongestStringSize => {
                self.print_longest_string_size(oss)?;
            }
            P::ShortestString => {
                self.print_shortest_string(oss)?;
            }
            P::ShortestStringSize => {
                self.print_shortest_string_size(oss)?;
            }
            P::Flags => {
                self.print_flags(oss);
            }
            P::Labels(opt) => match opt {
                Some(name) => {
                    self.print_labels_name(name, oss);
                }
                None => {
                    self.print_labels(oss);
                }
            },
            P::LabelCount => {
                self.print_label_count(oss);
            }
            P::LabelMaps => {
                self.print_labelmaps(oss);
            }
            P::Name => {
                self.print_name(oss);
            }
            P::Aliases => {
                self.print_aliases(oss);
            }
            P::Arccount => {
                self.print_arc_count(oss);
            }
            P::Defined => {
                self.print_defined(oss);
            }
            P::Dir => {
                self.print_dir("*", oss);
            }
            P::FileInfo => {
                self.print_file_info(oss);
            }
            P::List => {
                self.print_list(oss);
            }
            P::Lists => {
                self.print_list(oss);
            }
            P::Words(n) => {
                self.print_words("", n.unwrap_or(0), oss)?;
            }
            P::LowerWords(n) => {
                self.print_lower_words("", n.unwrap_or(0), oss)?;
            }
            P::UpperWords(n) => {
                self.print_upper_words("", n.unwrap_or(0), oss)?;
            }
            P::RandomWords(n) => {
                self.print_random_words("", n.unwrap_or(15), oss)?;
            }
            P::RandomLower(n) => {
                self.print_random_lower("", n.unwrap_or(15), oss)?;
            }
            P::RandomUpper(n) => {
                self.print_random_upper("", n.unwrap_or(15), oss)?;
            }
            P::Props => {
                self.print_properties(oss);
            }
        }
        Ok(())
    }

    // @brief Dispatch a parsed SaveCmd to the corresponding write_* method.
    // The save commands that the bison grammar wrote to a 'std::ofstream'
    // open the named file here; the ones that take a filename directly are
    // passed through.
    fn eval_save(&mut self, s: &nfst_xfst::SaveCmd) -> crate::error::Result<()> {
        use nfst_xfst::SaveCmd as S;
        match s {
            S::Stack(p) => {
                self.write_stack(p)?;
            }
            S::Definitions(p) => {
                self.write_definitions(p)?;
            }
            S::Definition(p) => {
                self.write_definition(p, "")?;
            }
            S::Prolog(p) => {
                if self.check_filename(p)
                    && let Ok(mut f) = std::fs::File::create(p)
                {
                    self.write_prolog(&mut f)?;
                }
            }
            S::Spaced(p) => {
                if self.check_filename(p)
                    && let Ok(mut f) = std::fs::File::create(p)
                {
                    self.write_spaced(&mut f);
                }
            }
            S::Text(p) => {
                if self.check_filename(p)
                    && let Ok(mut f) = std::fs::File::create(p)
                {
                    self.write_text(&mut f);
                }
            }
            S::Dot(p) => {
                if self.check_filename(p)
                    && let Ok(mut f) = std::fs::File::create(p)
                {
                    self.write_dot(&mut f);
                }
            }
            S::Att(p) => {
                if p.is_empty() {
                    let mut out = std::io::stdout();
                    self.write_att(&mut out);
                } else if self.check_filename(p)
                    && let Ok(mut f) = std::fs::File::create(p)
                {
                    self.write_att(&mut f);
                }
            }
        }
        Ok(())
    }

    // @brief Dispatch a parsed TestKind to the corresponding test_* method.
    // \a assertion is true when the test was wrapped in 'assert'.
    fn eval_test(
        &mut self,
        kind: nfst_xfst::TestKind,
        assertion: bool,
    ) -> crate::error::Result<()> {
        use nfst_xfst::TestKind as T;
        match kind {
            T::Eq => {
                self.test_eq(assertion)?;
            }
            T::Funct => {
                self.test_funct(assertion);
            }
            T::Id => {
                self.test_id(assertion)?;
            }
            T::Null => {
                self.test_null(false, assertion)?;
            }
            T::Nonnull => {
                self.test_nonnull(assertion)?;
            }
            T::Overlap => {
                self.test_overlap(assertion)?;
            }
            T::Sublanguage => {
                self.test_sublanguage(assertion)?;
            }
            T::Unambiguous => {
                self.test_unambiguous(assertion);
            }
            T::InfinitelyAmbiguous => {
                self.test_infinitely_ambiguous(assertion)?;
            }
            T::LowerBounded => {
                self.test_lower_bounded(assertion)?;
            }
            T::LowerUni => {
                self.test_lower_uni(assertion)?;
            }
            T::UpperBounded => {
                self.test_upper_bounded(assertion)?;
            }
            T::UpperUni => {
                self.test_upper_uni(assertion)?;
            }
        }
        Ok(())
    }
}
