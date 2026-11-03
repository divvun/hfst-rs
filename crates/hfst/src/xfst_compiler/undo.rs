//! Undoing a failed command, for a session that goes on after a failure.

use super::*;

// [spec:hfst:req:xfst-cmd.failed-command-no-effect]
/// What a command can change, saved before it runs. A network is saved only
/// when the command first changes it in place, so a command that builds new
/// networks copies nothing.
pub(super) struct Undo<B: AlgebraBackend> {
    stack: Vec<NetId>,
    definitions: BTreeMap<Symbol, NetId>,
    original_definitions: BTreeMap<Symbol, String>,
    function_definitions: BTreeMap<Symbol, String>,
    original_function_definitions: BTreeMap<Symbol, String>,
    function_arguments: BTreeMap<Symbol, u32>,
    names: BTreeMap<Symbol, NetId>,
    aliases: BTreeMap<Symbol, String>,
    variables: BTreeMap<String, String>,
    properties: BTreeMap<String, String>,
    lists: BTreeMap<Symbol, BTreeSet<Symbol>>,
    harmonize_flags: bool,
    engine_config: crate::hfst_transducer::EngineConfig,
    /// Networks from here on were made by the command and go with it.
    nets_len: usize,
    changed: BTreeMap<usize, HfstTransducer<B>>,
}

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    /// Save what `command` can change, when a failure would not end the run
    /// and no command around it is already saving. `source` and an alias are
    /// left to the commands they run.
    pub(super) fn begin_undo(&mut self, command: &nfst_xfst::XfstCommand) -> bool {
        if self.undo.is_some()
            || self.stops_on_failure()
            || matches!(
                command,
                nfst_xfst::XfstCommand::Source(_) | nfst_xfst::XfstCommand::Invoke(_)
            )
        {
            return false;
        }
        self.undo = Some(Undo {
            stack: self.stack.clone(),
            definitions: self.definitions.clone(),
            original_definitions: self.original_definitions.clone(),
            function_definitions: self.function_definitions.clone(),
            original_function_definitions: self.original_function_definitions.clone(),
            function_arguments: self.function_arguments.clone(),
            names: self.names.clone(),
            aliases: self.aliases.clone(),
            variables: self.variables.clone(),
            properties: self.properties.clone(),
            lists: self.lists.clone(),
            harmonize_flags: self.harmonize_flags,
            engine_config: self.engine_config,
            nets_len: self.nets.len(),
            changed: BTreeMap::new(),
        });
        true
    }

    /// Keep what the command did, or put back what it changed when it failed.
    pub(super) fn end_undo(&mut self, failed: bool) {
        let Some(undo) = self.undo.take() else {
            return;
        };
        if !failed {
            return;
        }
        self.stack = undo.stack;
        self.definitions = undo.definitions;
        self.original_definitions = undo.original_definitions;
        self.function_definitions = undo.function_definitions;
        self.original_function_definitions = undo.original_function_definitions;
        self.function_arguments = undo.function_arguments;
        self.names = undo.names;
        self.aliases = undo.aliases;
        self.variables = undo.variables;
        self.properties = undo.properties;
        self.lists = undo.lists;
        self.harmonize_flags = undo.harmonize_flags;
        self.engine_config = undo.engine_config;
        self.nets.truncate(undo.nets_len);
        for (slot, net) in undo.changed {
            self.nets[slot] = net;
        }
    }
}

impl<B: AlgebraBackend> XfstCompiler<B> {
    /// Save network `id` before the running command first changes it.
    pub(super) fn save_before_change(&mut self, id: NetId) {
        if let Some(undo) = &mut self.undo
            && id.0 < undo.nets_len
            && !undo.changed.contains_key(&id.0)
        {
            undo.changed.insert(id.0, self.nets[id.0].clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hfst_openfst::StdVectorFst;

    fn stack_after(script: &str) -> String {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        c.set_verbosity(true);
        let _ = c.parse(script);
        let _ = c.take_output();
        c.parse("print stack\n").expect("print stack runs");
        String::from_utf8(c.take_output()).expect("UTF-8 output")
    }

    // [spec:hfst:req:xfst-cmd.failed-command-no-effect/test]
    // shuffle failed after taking the top network off the stack and never
    // put it back.
    #[test]
    fn a_failed_shuffle_keeps_both_networks() {
        assert_eq!(
            stack_after("set quit-on-fail OFF\nregex a:b ;\nregex c:d ;\nshuffle net\n"),
            "0: 2 states, 1 arcs\n1: 2 states, 1 arcs\n"
        );
    }

    // [spec:hfst:req:xfst-cmd.failed-command-no-effect/test]
    // The fold changes the top network once per operand, so a failure on a
    // later operand must also undo the earlier steps.
    #[test]
    fn a_fold_failing_part_way_restores_every_network() {
        let script = "set quit-on-fail OFF\nregex b:c ;\nregex [a|b] ;\nregex a ;\nshuffle net\n";
        assert_eq!(
            stack_after(script),
            "0: 2 states, 1 arcs\n1: 2 states, 2 arcs\n2: 2 states, 1 arcs\n"
        );
    }

    // [spec:hfst:req:xfst-cmd.failed-command-no-effect/test]
    #[test]
    fn a_failed_command_keeps_definitions_and_variables() {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        c.parse("set quit-on-fail OFF\nregex a ;\ndefine X b ;\n")
            .expect("setup runs");
        let defined = c.definitions.clone();
        let variables = c.variables.clone();
        let _ = c.parse("shuffle net\n");
        assert_eq!(c.definitions, defined);
        assert_eq!(c.variables, variables);
    }

    // [spec:hfst:req:xfst-cmd.failed-command-no-effect/test]
    // A failure that ends the run keeps nothing for undoing.
    #[test]
    fn a_script_that_stops_on_failure_saves_nothing() {
        let mut c = XfstCompiler::<StdVectorFst>::new();
        let regex = nfst_xfst::parse("regex a ;").expect("parses");
        assert!(!c.begin_undo(&regex.value.commands[0].value));
        assert!(c.undo.is_none());
    }
}
