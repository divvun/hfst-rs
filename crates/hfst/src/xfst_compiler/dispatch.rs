//! The nfst-xfst command-dispatch driver, which replaces the bison action
//! dispatch.

use super::*;
use nfst_xfst::{
    ApplyKind, NetworkOp, ReadCmd, Redirect, RedirectKind, SubstituteCmd, TextSource, XfstCommand,
};

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:req:xfst-cmd.errors-are-values]
    /// Run one parsed command. A failure comes back as the error; `quit`
    /// comes back as `Flow::Quit`.
    pub(super) fn eval_command(&mut self, cmd: &XfstCommand) -> CmdResult<Flow> {
        match cmd {
            // ── regex / define ──────────────────────────────
            XfstCommand::Regex(xre) => self.eval_regex(xre)?,
            XfstCommand::Define { name, body } => self.eval_define(name, body)?,
            XfstCommand::DefineFunction { name, params, body } => {
                let prototype = format!("{}({})", name, params.join(", "));
                let xre = nfst_xre::pretty_print(body);
                self.define_function(&prototype, &xre)?;
            }
            XfstCommand::DefineAlias { name, body } => self.define_alias(name, body),
            XfstCommand::DefineList { name, members } => self.eval_define_list(name, members)?,
            XfstCommand::Undefine(names) => self.undefine(&names.join(" ")),
            XfstCommand::Unlist(name) => self.unlist(name),

            // ── stack ───────────────────────────────────────
            XfstCommand::Clear => self.clear(),
            XfstCommand::Pop => self.pop()?,
            XfstCommand::Push(name) => {
                if name.is_empty() {
                    self.push_latest()?;
                } else {
                    self.push(name)?;
                }
            }
            XfstCommand::Turn => self.turn(),
            XfstCommand::Rotate => self.rotate(),
            XfstCommand::LoadStack(name) => self.load_stack(name)?,
            XfstCommand::LoadDefinitions(name) => self.load_definitions(name)?,

            // ── network ops ─────────────────────────────────
            XfstCommand::Network(op) => self.eval_network_op(op)?,

            // ── apply / lookup ──────────────────────────────
            XfstCommand::Apply(kind, input) => self.eval_apply(kind, input)?,
            XfstCommand::LookupOptimize => self.lookup_optimize()?,
            XfstCommand::RemoveOptimization => self.remove_optimization()?,

            // ── read / save ─────────────────────────────────
            XfstCommand::Read(rc) => self.eval_read(rc)?,
            XfstCommand::Save(sc) => self.eval_save(sc)?,

            // ── print ───────────────────────────────────────
            XfstCommand::Print(p) => {
                let mut out = std::io::stdout();
                self.eval_print(p, &mut out)?;
            }

            // ── test ────────────────────────────────────────
            XfstCommand::Test(kind) => self.eval_test(*kind, false)?,

            // ── variables / show ────────────────────────────
            XfstCommand::Set { var, value } => self.eval_set(var, value)?,
            XfstCommand::Show(Some(name)) => self.show(name)?,
            XfstCommand::Show(None) => self.show_all(),
            XfstCommand::Echo(text) => self.echo(text),
            XfstCommand::System(command) => self.system(command)?,
            XfstCommand::Source(path) => return self.source_file(path),
            XfstCommand::Quit => {
                self.quit("bye");
                return Ok(Flow::Quit);
            }

            // ── substitute ──────────────────────────────────
            XfstCommand::Substitute(sub) => self.eval_substitute(sub)?,

            // ── help / misc ─────────────────────────────────
            XfstCommand::Apropos(opt) => self.apropos(opt.as_deref().unwrap_or("")),
            XfstCommand::Describe(text) => self.describe(text),
            XfstCommand::Assert(inner) => return self.eval_assert(&inner.value),
            XfstCommand::AddProps(content) => self.add_props(content)?,
            XfstCommand::EditProps => {
                return Err(CommandError::not_supported("edit properties"));
            }
            XfstCommand::Hfst(data) => self.hfst(data),
            // 'for' has no standalone production in the bison grammar.
            XfstCommand::For => self.prompt(),

            // ── i/o redirect wrapper ────────────────────────
            XfstCommand::Redirected { command, redirect } => {
                return self.eval_redirected(command, redirect);
            }
        }
        Ok(Flow::Continue)
    }

    /// The 'regex' command: compile the regex and push it.
    fn eval_regex(&mut self, xre: &nfst_xre::SpannedXre) -> CmdResult {
        let compiled = self.compile_spanned_xre(xre)?;
        let cfg = self.engine_config;
        self.net_mut(compiled).optimize_with_config(&cfg)?;
        self.stack.push(compiled);
        self.print_transducer_info();
        self.prompt();
        Ok(())
    }

    /// The 'define' command, with or without a regex body.
    fn eval_define(&mut self, name: &str, body: &nfst_xre::SpannedXre) -> CmdResult {
        // 'define NAME' / 'define NAME ;' with no regex body is the
        // C++ DEFINE_NAME production: it defines NAME as the network
        // popped from the top of the stack. nfst-xfst encodes the
        // missing body as Epsilon with an empty span, which is
        // distinguishable from an explicit epsilon regex ('define
        // NAME 0 ;', non-empty span).
        if body.span.is_empty() && matches!(body.value, nfst_xre::XreExpr::Epsilon) {
            return self.define(name);
        }
        let tr = self.compile_spanned_xre(body)?;
        self.define_transducer(name, tr);
        // 'print defined' reports from original_definitions.
        self.original_definitions
            .insert(Symbol::new(name), nfst_xre::pretty_print(body).to_string());
        self.prompt();
        Ok(())
    }

    /// The 'list' command: 'list NAME a-b' is a range, 'list NAME s1 s2 ...'
    /// a list.
    fn eval_define_list(&mut self, name: &str, members: &[String]) -> CmdResult {
        // A lone '-' is the symbol, not an empty range.
        if let [m] = members
            && m != "-"
            && let Some((start, end)) = m.split_once('-')
        {
            return self.define_list_by_range(name, start, end);
        }
        self.define_list(name, &members.join(" "))
    }

    /// Dispatch a parsed NetworkOp to the corresponding *_net method.
    fn eval_network_op(&mut self, op: &NetworkOp) -> CmdResult {
        match op {
            NetworkOp::Compose => self.compose_net(),
            NetworkOp::Concatenate => self.concatenate_net(),
            NetworkOp::Intersect => self.intersect_net(),
            NetworkOp::Union => self.union_net(),
            NetworkOp::Minus => self.minus_net(),
            NetworkOp::Crossproduct => self.crossproduct_net(),
            NetworkOp::Ignore => self.ignore_net(),
            NetworkOp::Invert => self.invert_net(),
            NetworkOp::Reverse => self.reverse_net(),
            NetworkOp::Determinize => self.determinize_net(),
            NetworkOp::Minimize => self.minimize_net(),
            NetworkOp::EpsilonRemove => self.epsilon_remove_net(),
            NetworkOp::PruneNet => self.prune_net(),
            NetworkOp::Negate => self.negate_net(),
            NetworkOp::OnePlus => self.one_plus_net(),
            NetworkOp::ZeroPlus => self.zero_plus_net(),
            NetworkOp::Sort => self.sort_net(),
            NetworkOp::Shuffle => self.shuffle_net(),
            NetworkOp::Substring => self.substring_net(),
            NetworkOp::Cleanup => self.cleanup_net(),
            NetworkOp::Complete => self.complete_net(),
            NetworkOp::LowerSide => self.lower_side_net(),
            NetworkOp::UpperSide => self.upper_side_net(),
            NetworkOp::Sigma => self.sigma_net(),
            NetworkOp::LabelNet => self.label_net(),
            NetworkOp::Inspect => self.inspect_net(),
            NetworkOp::TwosidedFlags => self.twosided_flags(),
            NetworkOp::EliminateAll => self.eliminate_flags(),
            NetworkOp::CollectEpsilonLoops => self.collect_epsilon_loops(),
            NetworkOp::CompactSigma => self.compact_sigma(),
            NetworkOp::View => self.view_net(),
            NetworkOp::ExtractAmbiguous => self.extract_ambiguous(),
            NetworkOp::ExtractUnambiguous => self.extract_unambiguous(),
            NetworkOp::Ambiguous => self.ambiguous_upper(),
            NetworkOp::CompileReplaceLower => self.compile_replace_lower_net(),
            NetworkOp::CompileReplaceUpper => self.compile_replace_upper_net(),
            NetworkOp::EliminateFlag(name) => self.eliminate_flag(name),
            NetworkOp::Name(name) => self.name_net(name),
        }
    }

    /// The 'apply up/down/med' commands, reading stdin when no input string
    /// was given.
    fn eval_apply(&mut self, kind: &ApplyKind, input: &Option<String>) -> CmdResult {
        let s: String = match input {
            Some(s) => s.clone(),
            None => {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                buf
            }
        };
        self.apply_text(kind, &s)
    }

    fn apply_text(&mut self, kind: &ApplyKind, s: &str) -> CmdResult {
        match kind {
            ApplyKind::Up => self.apply_up(s),
            ApplyKind::Down => self.apply_down(s),
            ApplyKind::Med => self.apply_med(s),
        }
    }

    /// Dispatch a parsed ReadCmd to the corresponding read_* method.
    fn eval_read(&mut self, rc: &ReadCmd) -> CmdResult {
        match rc {
            ReadCmd::Text(TextSource::Inline(s)) => self.read_text(s),
            ReadCmd::Text(TextSource::File(p)) => self.read_text_from_file(p),
            ReadCmd::Spaced(TextSource::Inline(s)) => self.read_spaced(s),
            ReadCmd::Spaced(TextSource::File(p)) => self.read_spaced_from_file(p),
            ReadCmd::Prolog(p) => {
                let s = self.read_input_file(p)?;
                self.read_prolog(&s)
            }
            ReadCmd::Props(p) => {
                let s = self.read_input_file(p)?;
                self.read_props(&s)
            }
            ReadCmd::Lexc(p) => self.read_lexc_from_file(p),
            ReadCmd::Att(p) => self.read_att_from_file(p),
        }
    }

    // [spec:hfst:req:xfst-cmd.io-errors]
    /// The whole of a file a command reads, or an error naming it.
    fn read_input_file(&self, path: &str) -> CmdResult<String> {
        self.check_filename(path)?;
        std::fs::read_to_string(path)
            .map_err(|e| CommandError::new(format!("could not read '{}': {}", path, e)))
    }

    // [spec:hfst:req:xfst-cmd.io-errors]
    /// A file a command writes to, created fresh or opened for appending.
    fn open_output_file(&self, path: &str, append: bool) -> CmdResult<std::fs::File> {
        self.check_filename(path)?;
        let opened = if append {
            std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
        } else {
            std::fs::File::create(path)
        };
        opened.map_err(|e| CommandError::new(format!("could not write '{}': {}", path, e)))
    }

    /// The 'set' command: a numeric value or a textual one.
    fn eval_set(&mut self, var: &str, value: &str) -> CmdResult {
        let trimmed = value.trim_start();
        let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
        match digits.parse::<u32>() {
            Ok(i) => self.set_number(var, i),
            Err(_) => self.set(var, value),
        }
    }

    /// Dispatch a parsed SubstituteCmd to the corresponding substitute_*
    /// method.
    fn eval_substitute(&mut self, sub: &SubstituteCmd) -> CmdResult {
        match sub {
            SubstituteCmd::Symbol { from, to, scope: _ } => {
                // substitute_symbol expects a quoted list: "s1" "s2" ...
                let list: String = from
                    .iter()
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join(" ");
                self.substitute_symbol(&list, to)
            }
            SubstituteCmd::Label { from, to, scope: _ } => {
                self.substitute_label(&from.join(" "), to)
            }
            SubstituteCmd::Named { def, label } => self.substitute_named(def, label),
        }
    }

    /// The 'assert' command. The bison grammar only allows 'assert TEST'
    /// productions, each of which calls test_X(true).
    fn eval_assert(&mut self, inner: &XfstCommand) -> CmdResult<Flow> {
        if let XfstCommand::Test(kind) = inner {
            self.eval_test(*kind, true)?;
            return Ok(Flow::Continue);
        }
        self.eval_command(inner)
    }

    /// Run a command under an i/o redirect: output to a file for '>' and
    /// '>>', input from a file for '<'. A command that has no output or input
    /// to redirect fails rather than ignoring the redirect.
    fn eval_redirected(
        &mut self,
        command: &nfst_xfst::Spanned<XfstCommand>,
        redirect: &Redirect,
    ) -> CmdResult<Flow> {
        let inner = &command.value;
        let path = &redirect.path;
        match (redirect.kind, inner) {
            (RedirectKind::Out | RedirectKind::Append, XfstCommand::Print(p)) => {
                let mut f = self.open_output_file(path, redirect.kind == RedirectKind::Append)?;
                self.eval_print(p, &mut f)?;
            }
            (RedirectKind::Out | RedirectKind::Append, XfstCommand::Save(s))
                if Self::save_writes_stream(s) =>
            {
                let mut f = self.open_output_file(path, redirect.kind == RedirectKind::Append)?;
                self.eval_save_to(s, &mut f)?;
            }
            (RedirectKind::In, XfstCommand::Apply(kind, _)) => {
                let s = self.read_input_file(path)?;
                self.apply_text(kind, &s)?;
            }
            (RedirectKind::In, XfstCommand::AddProps(_))
            | (RedirectKind::In, XfstCommand::Read(ReadCmd::Props(_))) => {
                let s = self.read_input_file(path)?;
                self.add_props(&s)?;
            }
            (RedirectKind::In, XfstCommand::Read(ReadCmd::Text(_))) => {
                self.read_text_from_file(path)?
            }
            (RedirectKind::In, XfstCommand::Read(ReadCmd::Spaced(_))) => {
                self.read_spaced_from_file(path)?
            }
            (RedirectKind::In, XfstCommand::LoadStack(_)) => self.load_stack(path)?,
            (RedirectKind::In, XfstCommand::LoadDefinitions(_)) => self.load_definitions(path)?,
            _ => {
                let text = self
                    .source
                    .get(command.span.range.clone())
                    .unwrap_or("this command")
                    .trim();
                return Err(CommandError::new(format!(
                    "'{}' cannot be redirected",
                    text
                )));
            }
        }
        Ok(Flow::Continue)
    }

    // @brief Compile a fully-parsed SpannedXre into a transducer by walking
    // it with self.xre. Mirrors the regex-compile path the bison actions used.
    // The XreCompiler::compile string entry point parses then walks the tree;
    // here the tree is already parsed, so we walk it directly and optimize,
    // returning a shared handle just like xre.compile.
    fn compile_spanned_xre(&mut self, xre: &nfst_xre::SpannedXre) -> CmdResult<NetId> {
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

    /// Dispatch a parsed PrintCmd to the corresponding print* method, writing
    /// to `oss` (stdout for plain commands, a file for redirected ones).
    fn eval_print(&mut self, p: &nfst_xfst::PrintCmd, oss: &mut dyn std::io::Write) -> CmdResult {
        use nfst_xfst::PrintCmd as P;
        match p {
            P::Net => self.print_net(oss),
            P::Stack => self.print_stack(oss),
            P::Sigma => self.print_sigma(oss, true),
            P::SigmaCount => self.print_sigma_count(oss),
            P::SigmaWordCount => self.print_sigma_word_count(oss),
            P::Size => self.print_size(oss),
            P::LongestString => self.print_longest_string(oss),
            P::LongestStringSize => self.print_longest_string_size(oss),
            P::ShortestString => self.print_shortest_string(oss),
            P::ShortestStringSize => self.print_shortest_string_size(oss),
            P::Flags => self.print_flags(oss),
            P::Labels(Some(name)) => self.print_labels_name(name, oss),
            P::Labels(None) => self.print_labels(oss),
            P::LabelCount => self.print_label_count(oss),
            P::LabelMaps => self.print_labelmaps(oss),
            P::Name => self.print_name(oss),
            P::Aliases => self.print_aliases(oss),
            P::Arccount => self.print_arc_count(oss),
            P::Defined => self.print_defined(oss),
            P::Dir => self.print_dir("*", oss),
            P::FileInfo => self.print_file_info(oss),
            P::List | P::Lists => self.print_list(oss),
            P::Words(n) => self.print_words("", n.unwrap_or(0), oss),
            P::LowerWords(n) => self.print_lower_words("", n.unwrap_or(0), oss),
            P::UpperWords(n) => self.print_upper_words("", n.unwrap_or(0), oss),
            P::RandomWords(n) => self.print_random_words("", n.unwrap_or(15), oss),
            P::RandomLower(n) => self.print_random_lower("", n.unwrap_or(15), oss),
            P::RandomUpper(n) => self.print_random_upper("", n.unwrap_or(15), oss),
            P::Props => self.print_properties(oss),
        }
    }

    /// Whether a save command writes a text stream (and so can go to a
    /// redirect) rather than a binary file it names itself.
    fn save_writes_stream(s: &nfst_xfst::SaveCmd) -> bool {
        use nfst_xfst::SaveCmd as S;
        matches!(
            s,
            S::Prolog(_) | S::Spaced(_) | S::Text(_) | S::Dot(_) | S::Att(_)
        )
    }

    /// Run a text-stream save command into `oss`.
    fn eval_save_to(&mut self, s: &nfst_xfst::SaveCmd, oss: &mut dyn std::io::Write) -> CmdResult {
        use nfst_xfst::SaveCmd as S;
        match s {
            S::Prolog(_) => self.write_prolog(oss),
            S::Spaced(_) => self.write_spaced(oss),
            S::Text(_) => self.write_text(oss),
            S::Dot(_) => self.write_dot(oss),
            S::Att(_) => self.write_att(oss),
            S::Stack(_) | S::Definition(_) | S::Definitions(_) => {
                unreachable!("binary saves name their own file")
            }
        }
    }

    /// Dispatch a parsed SaveCmd. The binary saves take their file name; the
    /// text saves write to the named file, or to stdout when none is given.
    fn eval_save(&mut self, s: &nfst_xfst::SaveCmd) -> CmdResult {
        use nfst_xfst::SaveCmd as S;
        match s {
            S::Stack(p) => self.write_stack(p),
            S::Definitions(p) => self.write_definitions(p),
            S::Definition(p) => self.write_definition(p, ""),
            S::Prolog(p) | S::Spaced(p) | S::Text(p) | S::Dot(p) | S::Att(p) => {
                if p.is_empty() {
                    let mut out = std::io::stdout();
                    self.eval_save_to(s, &mut out)
                } else {
                    let mut f = self.open_output_file(p, false)?;
                    self.eval_save_to(s, &mut f)
                }
            }
        }
    }

    /// Dispatch a parsed TestKind to the corresponding test_* method.
    /// `assertion` is true when the test was wrapped in 'assert'.
    fn eval_test(&mut self, kind: nfst_xfst::TestKind, assertion: bool) -> CmdResult {
        use nfst_xfst::TestKind as T;
        match kind {
            T::Eq => self.test_eq(assertion),
            T::Funct => self.test_funct(assertion),
            T::Id => self.test_id(assertion),
            T::Null => self.test_null(false, assertion),
            T::Nonnull => self.test_nonnull(assertion),
            T::Overlap => self.test_overlap(assertion),
            T::Sublanguage => self.test_sublanguage(assertion),
            T::Unambiguous => self.test_unambiguous(assertion),
            T::InfinitelyAmbiguous => self.test_infinitely_ambiguous(assertion),
            T::LowerBounded => self.test_lower_bounded(assertion),
            T::LowerUni => self.test_lower_uni(assertion),
            T::UpperBounded => self.test_upper_bounded(assertion),
            T::UpperUni => self.test_upper_uni(assertion),
        }
    }
}
