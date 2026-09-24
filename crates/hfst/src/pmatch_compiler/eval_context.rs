//! The per-compile evaluation context: construction, reset, and table accessors.

use super::*;

impl<B: AlgebraBackend + 'static> Default for PmatchEvalContext<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: AlgebraBackend + 'static> PmatchEvalContext<B> {
    pub fn new() -> Self {
        PmatchEvalContext {
            data: String::new(),
            len: 0,
            verbose: false,
            flatten: false,
            include_cosine_distances: false,
            timer: 0,
            minimization_guard_count: 0,
            named_object_evaluation_stack_depth: 0,
            need_delimiters: false,
            vector_similarity_projection_factor: 0.0,
            utils: None,
            pmatchnerrs: 0,
            definitions_table: BTreeMap::new(),
            variables: BTreeMap::new(),
            call_stack: Vec::new(),
            eval_stack: Vec::new(),
            def_insed_expressions: BTreeMap::new(),
            inserted_names: BTreeSet::new(),
            uncomposed: BTreeSet::new(),
            unsatisfied_insertions: BTreeSet::new(),
            used_definitions: BTreeSet::new(),
            function_names: BTreeSet::new(),
            capture_names: BTreeSet::new(),
            word_vectors: Vec::new(),
            named_transducers: BTreeMap::new(),
            includedir: String::new(),
            lst_line_map: BTreeMap::new(),
            lst_overlap_warned: BTreeSet::new(),
            node_caches: HashMap::new(),
        }
    }

    // Mirrors the former free 'init_globals': resets exactly the per-compile
    // subset the C++ reset, leaving 'uncomposed'/'word_vectors'/
    // 'named_transducers'/'includedir' alone (as the original did).
    pub(super) fn init_globals(&mut self) {
        self.definitions_table.clear();
        self.node_caches.clear();
        self.variables.clear();
        self.variables
            .insert("count-patterns".to_string(), "off".to_string());
        self.variables
            .insert("delete-patterns".to_string(), "off".to_string());
        self.variables
            .insert("extract-patterns".to_string(), "off".to_string());
        self.variables
            .insert("locate-patterns".to_string(), "off".to_string());
        self.variables
            .insert("mark-patterns".to_string(), "on".to_string());
        self.variables
            .insert("max-context-length".to_string(), "254".to_string());
        self.variables
            .insert("max-recursion".to_string(), "5000".to_string());
        self.variables
            .insert("need-separators".to_string(), "on".to_string());
        self.variables
            .insert("unicode-character-classes".to_string(), "off".to_string());
        self.variables
            .insert("xerox-composition".to_string(), "on".to_string());
        self.variables.insert(
            "vector-similarity-projection-factor".to_string(),
            "1.0".to_string(),
        );
        self.call_stack.clear();
        self.eval_stack.clear();
        self.def_insed_expressions.clear();
        self.inserted_names.clear();
        self.unsatisfied_insertions.clear();
        self.used_definitions.clear();
        self.function_names.clear();
        self.capture_names.clear();
        self.zero_minimization_guard();
        self.named_object_evaluation_stack_depth = 0;
        self.need_delimiters = false;
        self.pmatchnerrs = 0;
        self.lst_line_map.clear();
        self.lst_overlap_warned.clear();
    }

    // --- NODE_CACHES (off-node memoization; replaces PmatchObject::cache) ---
    pub(super) fn node_cache_get(&self, key: usize) -> Option<&HfstTransducer<B>> {
        self.node_caches.get(&key)
    }
    pub(super) fn node_cache_get_mut(&mut self, key: usize) -> Option<&mut HfstTransducer<B>> {
        self.node_caches.get_mut(&key)
    }
    pub(super) fn node_cache_put(&mut self, key: usize, t: HfstTransducer<B>) {
        self.node_caches.insert(key, t);
    }

    // --- DEFINITIONS (BTreeMap<String, ObjRef>) ---
    pub(super) fn definitions_get(&self, k: &str) -> Option<ObjRef<B>> {
        self.definitions_table.get(k).cloned()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-in-global-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-in-global-context-fn]
    pub(super) fn definitions_contains(&self, k: &str) -> bool {
        self.definitions_table.contains_key(k)
    }
    pub(super) fn definitions_insert(&mut self, k: String, v: ObjRef<B>) {
        self.definitions_table.insert(k, v);
    }
    fn definitions_clear(&mut self) {
        self.definitions_table.clear();
    }
    pub(super) fn definitions_len(&self) -> usize {
        self.definitions_table.len()
    }
    pub(super) fn definitions_is_empty(&self) -> bool {
        self.definitions_table.is_empty()
    }
    pub(super) fn definitions_keys(&self) -> Vec<String> {
        self.definitions_table.keys().cloned().collect()
    }
    pub(super) fn definitions_snapshot(&self) -> Vec<(String, ObjRef<B>)> {
        self.definitions_table
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    // --- DEF_INSED_EXPRESSIONS (BTreeMap<String, ObjRef>) ---
    pub(super) fn def_insed_expressions_get(&self, k: &str) -> Option<ObjRef<B>> {
        self.def_insed_expressions.get(k).cloned()
    }
    pub(super) fn def_insed_expressions_contains(&self, k: &str) -> bool {
        self.def_insed_expressions.contains_key(k)
    }
    pub(super) fn def_insed_expressions_insert(&mut self, k: String, v: ObjRef<B>) {
        self.def_insed_expressions.insert(k, v);
    }
    fn def_insed_expressions_clear(&mut self) {
        self.def_insed_expressions.clear();
    }
    pub(super) fn def_insed_expressions_len(&self) -> usize {
        self.def_insed_expressions.len()
    }
    pub(super) fn def_insed_expressions_is_empty(&self) -> bool {
        self.def_insed_expressions.is_empty()
    }

    // --- VARIABLES (BTreeMap<String, String>) ---
    pub(super) fn variables_get(&self, k: &str) -> Option<String> {
        self.variables.get(k).cloned()
    }
    pub(super) fn variables_insert(&mut self, k: String, v: String) {
        self.variables.insert(k, v);
    }
    fn variables_clear(&mut self) {
        self.variables.clear();
    }
    pub(super) fn variables_snapshot(&self) -> Vec<(String, String)> {
        self.variables
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
    pub(super) fn variables_entry_or_default(&mut self, k: &str) -> String {
        self.variables.entry(k.to_string()).or_default().clone()
    }
    pub(super) fn variables_index(&self, k: &str) -> String {
        self.variables[k].clone()
    }

    // --- CALL_STACK (Vec<BTreeMap<String, ObjRef>>) ---
    pub(super) fn call_stack_len(&self) -> usize {
        self.call_stack.len()
    }
    pub(super) fn call_stack_last_get(&self, k: &str) -> Option<ObjRef<B>> {
        self.call_stack.last().and_then(|f| f.get(k).cloned())
    }
    pub(super) fn call_stack_last_clone(&self) -> BTreeMap<String, ObjRef<B>> {
        self.call_stack
            .last()
            .expect("call_stack has a frame during evaluation")
            .clone()
    }
    pub(super) fn call_stack_push(&mut self, frame: BTreeMap<String, ObjRef<B>>) {
        self.call_stack.push(frame);
    }
    pub(super) fn call_stack_pop(&mut self) {
        self.call_stack.pop();
    }
    fn call_stack_clear(&mut self) {
        self.call_stack.clear();
    }

    // --- EVAL_STACK (Vec<String>) ---
    pub(super) fn eval_stack_push(&mut self, v: String) {
        self.eval_stack.push(v);
    }
    pub(super) fn eval_stack_pop(&mut self) {
        self.eval_stack.pop();
    }
    pub(super) fn eval_stack_last(&self) -> Option<String> {
        self.eval_stack.last().cloned()
    }
    fn eval_stack_clear(&mut self) {
        self.eval_stack.clear();
    }

    // --- utility-transducer cache (formerly the 'UTILS' thread-local) ---
    // The cache is constructed on first use. The utility methods invoked inside
    // 'f' never re-enter 'with_utils', so the '&mut self' borrow held across 'f'
    // cannot double-borrow.
    pub(super) fn with_utils<R>(
        &mut self,
        f: impl FnOnce(&mut PmatchUtilityTransducers<B>) -> crate::error::Result<R>,
    ) -> crate::error::Result<R> {
        if self.utils.is_none() {
            self.utils = Some(PmatchUtilityTransducers::new()?);
        }
        f(self.utils.as_mut().expect("utils set just above"))
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.zero-minimization-guard-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.zero-minimization-guard-fn]
    pub(super) fn zero_minimization_guard(&mut self) {
        self.minimization_guard_count = 0;
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.make-minimization-guard-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-minimization-guard-fn]
    pub(super) fn make_minimization_guard(
        &mut self,
    ) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
        let mut guard = String::new();
        if self.minimization_guard_count == 0 {
            guard.push_str(internal_epsilon);
        } else {
            let mgc = self.minimization_guard_count;
            guard.push_str(&format!("@PMATCH_GUARD_{}@", mgc));
        }
        self.minimization_guard_count += 1;
        epsilon_to_symbol_container(self, guard)
    }

    // [spec:hfst:def:pmatch-utils.pmatcherror-fn]
    // [spec:hfst:sem:pmatch-utils.pmatcherror-fn]
    pub(super) fn pmatcherror(&self, msg: &str) {
        let buf = self.data.clone();
        let bytes = buf.as_bytes();
        let parsedata: String = if bytes.is_empty() {
            String::new()
        } else if bytes.len() < 60 {
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            String::from_utf8_lossy(&bytes[..59]).into_owned() + "... [truncated]"
        };
        let mut errmsg = String::new();
        errmsg.push_str("hfst-pmatch:");
        errmsg.push_str("parsing failed: ");
        errmsg.push_str(msg);
        errmsg.push_str("\n*** parsing ");
        errmsg.push_str(&parsedata);
        errmsg.push('\n');

        std::panic::panic_any(errmsg);
    }
}
