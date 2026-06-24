# back-ends/sfst/operators.cc

> [spec:hfst:def:operators.sfst.add-transition-fn]
> static void add_transition( Label l, Node *n1, Node *n2, Node *node,

> [spec:hfst:sem:operators.sfst.add-transition-fn]
> Adds a transition labelled `l` into the composed transducer `a`, where the
> destination corresponds to the source-node pair `(n1, n2)`. Looks up `(n1, n2)`
> in `map` (a PairMapping). If found, simply adds an arc with label `l` from
> `node` to the already-mapped target node and returns. Otherwise creates a new
> node `target_node` in `a` via `a->new_node()`, records the mapping
> `map[(n1,n2)] = target_node`, adds an arc with label `l` from `node` to
> `target_node`, then recurses by calling `compose_nodes(n1, n2, target_node, a,
> map, cn2trans1, cn2trans2)` to build out the rest of the composition from that
> pair. Returns nothing.

> [spec:hfst:def:operators.sfst.char-node2-trans]
> class CharNode2Trans {
>   struct hashf { // [spec:hfst:def:operators.sfst.char-node2-trans.hashf.operator-fn] // [spec:hfst:sem:operators.sfst.char-node2-trans.hashf.operator-fn] size...;
>   struct equalf { // [spec:hfst:def:operators.sfst.char-node2-trans.equalf.operator-fn] // [spec:hfst:sem:operators.sfst.char-node2-trans.equalf.operator-fn] i...;
>   Transducer &transducer;
>   vector<Index> node_size;
>   vector<Arc*> cs_transitions;
>   NodeSym2Range trange;
>   class iterator { CharNode2Trans &c2t; Index current, end; public: // [spec:hfst:def:operators.sfst.char-node2-trans.iterator.iterator-fn] // [spec:hfst:sem:o...;
> }

> [spec:hfst:def:operators.sfst.char-node2-trans.char-node2-trans-fn]
> CharNode2Trans::CharNode2Trans(Transducer &t): transducer(t)

> [spec:hfst:sem:operators.sfst.char-node2-trans.char-node2-trans-fn]
> Constructor. Stores the reference to transducer `t` in the member `transducer`.
> Calls `transducer.nodeindexing()` which returns a `pair<Index,Index>` of
> `(node_count, transition_count)`. Resizes the `node_size` vector to
> `node_count` entries, each initialised to the sentinel `undef`. Reserves
> capacity for `transition_count` entries in the `cs_transitions` vector (reserve
> only, does not change its size). Leaves `trange` empty.

> [spec:hfst:def:operators.sfst.char-node2-trans.equalf]
> struct equalf

> [spec:hfst:def:operators.sfst.char-node2-trans.equalf.operator-fn]
> int operator()(const NodeSym &ns1, const NodeSym &ns2) const

> [spec:hfst:sem:operators.sfst.char-node2-trans.equalf.operator-fn]
> Equality functor for `NodeSym` keys in the hash map. Returns true (as an int)
> iff both fields match: `ns1.nodeID == ns2.nodeID && ns1.symbol == ns2.symbol`.

> [spec:hfst:def:operators.sfst.char-node2-trans.hash-transitions-fn]
> size_t CharNode2Trans::hash_transitions( Node *node, bool upper )

> [spec:hfst:sem:operators.sfst.char-node2-trans.hash-transitions-fn]
> Indexes the outgoing arcs of `node` by character, on the upper layer if
> `upper` is true, otherwise the lower layer, and returns the number of distinct
> characters. First checks the memo `node_size[node->index]`: if it is not
> `undef`, returns that cached count immediately (idempotent — already indexed).
> Otherwise builds a local `Sym2Arcs` map (`map<Character, vector<Arc*>>`):
> iterates over all outgoing arcs of `node`, and for each arc appends the arc
> pointer to the bucket keyed by `arc->label().upper_char()` (if `upper`) or
> `arc->label().lower_char()` (if not). Then iterates the `Sym2Arcs` map in key
> order; for each character `sym` with its vector of arcs, it records a `FromTo`
> range: `range.from` is the current size of `cs_transitions`, then pushes each
> arc pointer in the vector onto `cs_transitions`, then sets `range.to` to the
> new size, and stores `trange[NodeSym(node->index, sym)] = range`. Finally sets
> `n = sym2arcs.size()`, caches it as `node_size[node->index] = n`, and returns
> `n`.

> [spec:hfst:def:operators.sfst.char-node2-trans.hashf]
> struct hashf

> [spec:hfst:def:operators.sfst.char-node2-trans.hashf.operator-fn]
> size_t operator()(const NodeSym &ns) const

> [spec:hfst:sem:operators.sfst.char-node2-trans.hashf.operator-fn]
> Hash functor for `NodeSym` keys. Returns the bitwise XOR of the two fields:
> `ns.nodeID ^ ns.symbol`, as a `size_t`.

> [spec:hfst:def:operators.sfst.char-node2-trans.iterator]
> class iterator {
>   CharNode2Trans &c2t;
>   Index current, end;
> }

> [spec:hfst:def:operators.sfst.char-node2-trans.iterator.finished-fn]
> bool finished()

> [spec:hfst:sem:operators.sfst.char-node2-trans.iterator.finished-fn]
> Returns true iff the iterator has reached the end of its range, i.e.
> `current == end`.

> [spec:hfst:def:operators.sfst.char-node2-trans.iterator.iterator-fn]
> iterator( CharNode2Trans &table, Index nodeID, Character symbol )

> [spec:hfst:sem:operators.sfst.char-node2-trans.iterator.iterator-fn]
> Constructs an iterator over the transitions of node `nodeID` carrying
> character `symbol`. Stores the reference to the `CharNode2Trans` table `table`
> in member `c2t`. Looks up `c2t.trange[NodeSym(nodeID, symbol)]` to get a
> `FromTo` range (note: this uses `operator[]` on the hash map, so a missing key
> inserts a default-constructed `FromTo`, typically yielding `from==to` so the
> iterator is immediately finished). Sets `current = range.from` and
> `end = range.to`. The iterated arc pointers live in `c2t.cs_transitions`.

> [spec:hfst:def:operators.sfst.char-node2-trans.iterator.operator-fn]
> void operator++( int )

> [spec:hfst:sem:operators.sfst.char-node2-trans.iterator.operator-fn]
> Post-increment operator (takes a dummy int). Advances the iterator by
> incrementing `current` by one. Returns nothing.

> [spec:hfst:def:operators.sfst.char-node2-trans.iterator.size-fn]
> Index size()

> [spec:hfst:sem:operators.sfst.char-node2-trans.iterator.size-fn]
> Returns the number of transitions remaining in the iterator from the current
> position, i.e. `end - current`.

> [spec:hfst:def:operators.sfst.char-node2-trans.node-sym2-range]
> typedef hash_map<NodeSym, FromTo, hashf, equalf > NodeSym2Range

> [spec:hfst:def:operators.sfst.check-cyclicity-fn]
> static bool check_cyclicity( Node *node, NodeHashSet &visited,

> [spec:hfst:sem:operators.sfst.check-cyclicity-fn]
> Detects an epsilon-only cycle reachable from `node`, following only arcs whose
> upper character is epsilon, using `visited` as the current DFS path set.
> Attempts to insert `node` into `visited`; if insertion fails (i.e. `node` is
> already on the current path), returns true immediately (a cycle was found).
> Otherwise iterates the outgoing arcs of `node`; for each arc whose label has
> `upper_is_epsilon()` true, recurses into `arc->target_node()`. If a recursive
> call returns true, this writes `alphabet.write_label(arc->label())` followed by
> a newline to `cerr` (building up the cycle's label trace) and returns true. If
> no cycle is found, erases `node` from `visited` (backtracking) and returns
> false. `alphabet` is read-only here (used only to format labels).

> [spec:hfst:def:operators.sfst.cn2trans2-fn]
> CharNode2Trans cn2trans2(a)

> [spec:hfst:sem:operators.sfst.cn2trans2-fn]
> Within `operator||`, constructs the second `CharNode2Trans` index, `cn2trans2`,
> over the right-hand transducer `a` (passed by reference to the constructor).
> This sets up the per-node/per-character transition index used to look up `a`'s
> lower-side matching arcs during composition. See the `CharNode2Trans`
> constructor for what construction does. The companion `cn2trans1` is built over
> `*this`. Both are then passed into `compose_nodes`.

> [spec:hfst:def:operators.sfst.compose-nodes-fn]
> static void compose_nodes( Node *n1, Node *n2, Node *node, Transducer *a,

> [spec:hfst:sem:operators.sfst.compose-nodes-fn]
> Builds the composition of node `n1` (from the left transducer) with node `n2`
> (from the right transducer) into `node` of the result transducer `a`, using
> `map` to dedupe node pairs and `cn2trans1`/`cn2trans2` as transition indexes.
> First indexes `n1`'s upper-side arcs via `size1 = cn2trans1.hash_transitions(n1,
> true)` and `n2`'s lower-side arcs via `size2 = cn2trans2.hash_transitions(n2,
> false)`. Chooses to drive the iteration off `n1`'s arc list and hash-look-up in
> `cn2trans2` when `size1 <= size2` (flag `hash2 = (size1 <= size2)`), else the
> reverse. If both `n1` and `n2` are final, sets `node` final.
>
> When `hash2` is true: iterates every outgoing arc of `n1`. Let `t1` be its
> target, `l1` its label, `uc1`/`lc1` its upper/lower characters. If
> `uc1 == Label::epsilon`, calls `add_transition(l1, t1, n2, node, a, map,
> cn2trans1, cn2trans2)` (epsilon on the input side advances only `n1`).
> Otherwise iterates the matching arcs of `n2` via
> `CharNode2Trans::iterator(cn2trans2, n2->index, uc1)`; for each matched arc
> `arc2` (target `t2`, label `l2`, with asserted `uc1 == l2.lower_char()`,
> `uc2 = l2.upper_char()`), calls `add_transition(Label(lc1, uc2), t1, t2,
> node, ...)`. After that loop, handles `n2`'s lower-epsilon arcs: iterates
> `CharNode2Trans::iterator(cn2trans2, n2->index, Label::epsilon)` (asserting
> `l.lower_char() == Label::epsilon`) and calls `add_transition(l, n1, t2, node,
> ...)` (advances only `n2`).
>
> When `hash2` is false: the symmetric case driven by `n2`'s arcs. Iterates
> every outgoing arc of `n2` (target `t2`, label `l2`, `uc2`/`lc2`). If
> `lc2 == Label::epsilon`, calls `add_transition(l2, n1, t2, node, ...)`.
> Otherwise iterates matching arcs of `n1` via
> `CharNode2Trans::iterator(cn2trans1, n1->index, lc2)` (asserting
> `l1.upper_char() == lc2`, `lc1 = l1.lower_char()`) and calls
> `add_transition(Label(lc1, uc2), t1, t2, node, ...)`. Then handles `n1`'s
> upper-epsilon arcs via `CharNode2Trans::iterator(cn2trans1, n1->index,
> Label::epsilon)` (asserting `l.upper_char() == Label::epsilon`) and calls
> `add_transition(l, t1, n2, node, ...)`. Returns nothing; all node/arc creation
> happens via `add_transition`'s recursion.

> [spec:hfst:def:operators.sfst.conjoin-nodes-fn]
> static void conjoin_nodes( Node *n1, Node *n2, Node *node,

> [spec:hfst:sem:operators.sfst.conjoin-nodes-fn]
> Builds the intersection (conjunction) of node `n1` with node `n2` into `node`
> of result transducer `a`, using `map` (PairMapping) to dedupe node pairs. If
> both `n1` and `n2` are final, sets `node` final. Iterates over every outgoing
> arc of `n1`: let `l` be its label, `t1` its target. Looks up
> `t2 = n2->target_node(l)` — the target of `n2`'s arc with the identical label
> `l`. If `t2` is null (no matching arc), skips this arc. Otherwise looks up the
> pair `(t1, t2)` in `map`: if absent, creates a new node `target_node` in `a`,
> records `map[(t1,t2)] = target_node`, adds an arc labelled `l` from `node` to
> `target_node`, and recurses with `conjoin_nodes(t1, t2, target_node, a, map)`;
> if present, just adds an arc labelled `l` from `node` to the already-mapped
> node `it->second`. Returns nothing.

> [spec:hfst:def:operators.sfst.from-to]
> class FromTo {
>   Index from, to;
> }

> [spec:hfst:def:operators.sfst.from-to.size-fn]
> Index size()

> [spec:hfst:sem:operators.sfst.from-to.size-fn]
> Returns the width of the transition range, i.e. `to - from`.

> [spec:hfst:def:operators.sfst.node-sym]
> class NodeSym {
>   Index nodeID;
>   Character symbol;
> }

> [spec:hfst:def:operators.sfst.node-sym.node-sym-fn]
> NodeSym( Index n, Character s )

> [spec:hfst:sem:operators.sfst.node-sym.node-sym-fn]
> Constructor for the (node, symbol) pair. Stores `nodeID = n` and
> `symbol = s`.

> [spec:hfst:def:operators.sfst.sym2-arcs]
> typedef map<Character, vector<Arc*> > Sym2Arcs

> [spec:hfst:def:operators.sfst.transducer.compare-nodes-fn]
> bool Transducer::compare_nodes( Node *node, Node *node2, Transducer &a2 )

> [spec:hfst:sem:operators.sfst.transducer.compare-nodes-fn]
> Recursively tests whether the subgraph at `node` (in `this`) is isomorphic to
> the subgraph at `node2` (in `a2`), assuming both transducers are deterministic
> (matching is by exact label). Uses the `forward` pointers as a bijective
> pairing and the visited marks `vmark`/`a2.vmark`.
>
> Visited handling: if `node` was already visited (under `this`'s `vmark`): if
> `node2` was also visited (under `a2.vmark`), return whether they were paired
> with each other — `node->forward() == node2 && node2->forward() == node` —
> else return false. If `node` was not visited but `node2` was, return false.
>
> Otherwise (first visit of both): pair them by `node->set_forward(node2)` and
> `node2->set_forward(node)` (this also marks them visited). If their finality
> differs (`node->is_final() != node2->is_final()`), return false. Then for each
> outgoing arc of `node`, find `t2 = node2->target_node(arc->label())`; if null,
> return false; otherwise recurse `compare_nodes(arc->target_node(), t2, a2)`,
> returning false if it fails. After that, for each outgoing arc of `node2`,
> ensure `node->target_node(arc->label())` is non-null (so `node2` has no extra
> labels), returning false otherwise. If all checks pass, return true.

> [spec:hfst:def:operators.sfst.transducer.copy-nodes-fn]
> Node *Transducer::copy_nodes( Node *node, Transducer *a,

> [spec:hfst:sem:operators.sfst.transducer.copy-nodes-fn]
> Recursively copies the subgraph rooted at `node` into transducer `a`, returning
> the freshly created counterpart node. Uses `forward` pointers + `vmark` for
> memoisation. If `node` has not been visited under `vmark`: create a new node in
> `a` via `a->new_node()` and store it as `node->set_forward(...)` (also marking
> visited). If `node->is_final()`, mark the new node final. Then for each outgoing
> arc of `node`: recurse `copy_nodes(arc->target_node(), a, lswitch, recode)` to
> get the copied target node `tn`; compute the (possibly transformed) label via
> `recode_label(arc->label(), lswitch, recode, a->alphabet)`; and add that arc
> from `node->forward()` to `tn` in `a`. In all cases (visited or not), return
> `node->forward()`. Parameters `lswitch` (swap upper/lower) and `recode` (remap
> symbols into `a`'s alphabet) are passed through to `recode_label`.

> [spec:hfst:def:operators.sfst.transducer.freely-insert-at-node-fn]
> void Transducer::freely_insert_at_node( Node *node, Label l )

> [spec:hfst:sem:operators.sfst.transducer.freely-insert-at-node-fn]
> Adds a self-loop labelled `l` to every node reachable from `node`. If `node`
> has not been visited under `vmark`: add a recursive arc with label `l` from
> `node` back to `node` itself (via `node->add_arc(l, node, this)`); the
> visited mark for `node` is set as part of the `was_visited` check. Then iterate
> over all outgoing arcs of `node` (which now includes the newly added self-loop,
> but recursion into `node` is guarded by the visited check) and recurse into
> each `arc->target_node()`. Returns nothing.

> [spec:hfst:def:operators.sfst.transducer.generates-empty-string-fn]
> bool Transducer::generates_empty_string()

> [spec:hfst:sem:operators.sfst.transducer.generates-empty-string-fn]
> Returns whether the empty string is in the transducer's language, i.e. whether
> the root node is final. If `this` is not yet `minimised`, computes a minimised
> copy via `minimise()`, reads `tmp->root_node()->is_final()` into a result,
> deletes the temporary transducer, and returns that result. Otherwise (already
> minimised) returns `root_node()->is_final()` directly.

> [spec:hfst:def:operators.sfst.transducer.infinitely-ambiguous-node-fn]
> bool Transducer::infinitely_ambiguous_node( Node *node )

> [spec:hfst:sem:operators.sfst.transducer.infinitely-ambiguous-node-fn]
> Recursively tests whether the subgraph at `node` contains an epsilon
> (upper-side) cycle, which would make the transducer infinitely ambiguous. Uses
> `vmark` for memoisation. If `node` has not yet been visited under `vmark`
> (the `was_visited(vmark)` call also marks it): create a fresh empty
> `NodeHashSet visited`, and call `check_cyclicity(node, visited, alphabet)`; if
> that returns true, return true. Otherwise iterate over all outgoing arcs of
> `node` and recurse into each `arc->target_node()`; if any recursion returns
> true, return true. If `node` was already visited or nothing found, return
> false.

> [spec:hfst:def:operators.sfst.transducer.is-automaton-fn]
> bool Transducer::is_automaton()

> [spec:hfst:sem:operators.sfst.transducer.is-automaton-fn]
> Returns whether the transducer is an automaton (i.e. every transition has equal
> upper and lower characters, so it represents an identity relation / acceptor).
> Calls `incr_vmark()` to start a fresh traversal generation, then returns
> `is_automaton_node(root_node())`.

> [spec:hfst:def:operators.sfst.transducer.is-automaton-node-fn]
> bool Transducer::is_automaton_node( Node *node )

> [spec:hfst:sem:operators.sfst.transducer.is-automaton-node-fn]
> Recursively tests whether the subgraph at `node` is an automaton (all arcs have
> equal upper and lower characters). If `node` has not been visited under `vmark`
> (the `was_visited(vmark)` call marks it): iterate over all outgoing arcs; for
> each arc with label `l`, if `l.upper_char() != l.lower_char()` return false;
> otherwise recurse into `arc->target_node()`, returning false if the recursion
> returns false. If `node` was already visited or all arcs pass, return true.
> Note: because the visited mark is set at entry, an already-visited node short-
> circuits to true without re-checking its arcs.

> [spec:hfst:def:operators.sfst.transducer.is-cyclic-fn]
> bool Transducer::is_cyclic()

> [spec:hfst:sem:operators.sfst.transducer.is-cyclic-fn]
> Returns whether the transducer's graph contains any directed cycle. Calls
> `incr_vmark()` to start a fresh traversal generation, creates an empty
> `NodeHashSet previous` (the current DFS path), and returns
> `is_cyclic_node(root_node(), previous)`.

> [spec:hfst:def:operators.sfst.transducer.is-cyclic-node-fn]
> bool Transducer::is_cyclic_node( Node *node, NodeHashSet &previous )

> [spec:hfst:sem:operators.sfst.transducer.is-cyclic-node-fn]
> Recursively detects a cycle reachable from `node`, with `previous` holding the
> set of nodes on the current DFS path. If `node` has not been visited under
> `vmark` (the `was_visited(vmark)` call marks it as fully explored): inserts
> `node` into `previous`, remembering the resulting iterator `it`. Declares an
> unused local `NodeHashSet visited`. Iterates over all outgoing arcs; for each,
> if the target node is already in `previous` (a back edge → cycle) OR a
> recursive `is_cyclic_node(arc->target_node(), previous)` returns true, returns
> true. After processing all arcs without finding a cycle, erases `node` from
> `previous` (via `it`) and falls through. If `node` was already visited or no
> cycle was found, returns false.

> [spec:hfst:def:operators.sfst.transducer.is-empty-fn]
> bool Transducer::is_empty()

> [spec:hfst:sem:operators.sfst.transducer.is-empty-fn]
> Returns whether the transducer's language is empty. If `this` is not yet
> `minimised`, computes a minimised copy via `minimise()`, recursively calls
> `is_empty()` on it into a result, deletes the temporary, and returns that
> result. Otherwise (already minimised): if `root_node()->is_final()`, return
> false (the empty string is accepted, so language is non-empty); else return
> `root_node()->arcs()->is_empty()` (empty iff the root has no outgoing arcs).

> [spec:hfst:def:operators.sfst.transducer.is-infinitely-ambiguous-fn]
> bool Transducer::is_infinitely_ambiguous()

> [spec:hfst:sem:operators.sfst.transducer.is-infinitely-ambiguous-fn]
> Returns whether the transducer is infinitely ambiguous (contains an
> epsilon/upper cycle). Calls `incr_vmark()` to start a fresh traversal
> generation, then returns `infinitely_ambiguous_node(root_node())`.

> [spec:hfst:def:operators.sfst.transducer.map-nodes-fn]
> void Transducer::map_nodes( Node *node, Node *node2, Transducer *a, Level level)

> [spec:hfst:sem:operators.sfst.transducer.map-nodes-fn]
> Recursively projects the subgraph at `node` to a single level (input or output,
> per `level`) into transducer `a`, mirroring the structure onto `node2`. Uses
> `forward` pointers + `vmark` for memoisation. If `node` has not been visited
> under `vmark`: pair it via `node->set_forward(node2)` (marks visited). If
> `node->is_final()`, mark `node2` final. Then for each outgoing arc of `node`:
> build the projected label `l(arc->label().get_char(level))` (single-character
> label on the chosen level). Determine the counterpart target `t2`: let
> `t = arc->target_node()`; if `t->check_visited(vmark)` is true, reuse
> `t2 = t->forward()`; otherwise create `t2 = a->new_node()`. Add an arc with
> label `l` from `node2` to `t2` in `a`, then recurse
> `map_nodes(t, t2, a, level)`. Returns nothing.

> [spec:hfst:def:operators.sfst.transducer.negate-nodes-fn]
> void Transducer::negate_nodes( Node *node, Node *accept )

> [spec:hfst:sem:operators.sfst.transducer.negate-nodes-fn]
> Recursively complements the (deterministic, minimised) transducer in place by
> flipping finality and completing the transition function with a sink `accept`
> node. If `node` has not been visited under `vmark` (the `was_visited(vmark)`
> call marks it): toggle finality via `node->set_final(!node->is_final())`. Then
> iterate over all existing outgoing arcs of `node` and recurse into each
> `arc->target_node()` first (so recursion sees the original arc set). After
> that, iterate over every label in the transducer's `alphabet`; for any label
> `*it` for which `node->target_node(*it)` is null (no existing transition on
> that label), add an arc with that label from `node` to the universal
> accepting node `accept`. Returns nothing.

> [spec:hfst:def:operators.sfst.transducer.operator-fn]
> bool Transducer::operator==( Transducer &a )

> [spec:hfst:sem:operators.sfst.transducer.operator-fn]
> Equality operator: returns whether `this` and `a` denote the same relation.
> Obtains minimised forms `p1` (`= this` if already minimised, else
> `&minimise()`) and `p2` (`= &a` if already minimised, else `&a.minimise()`).
> Calls `p1->incr_vmark()` and `p2->incr_vmark()` to start fresh traversal
> generations, then computes
> `result = p1->compare_nodes(p1->root_node(), p2->root_node(), *p2)`. Frees any
> temporaries: if `p1 != this` delete `p1`; if `p2 != &a` delete `p2`. Returns
> `result`.

> [spec:hfst:def:operators.sfst.transducer.rec-cat-nodes-fn]
> void Transducer::rec_cat_nodes( Node *node, Node *node2 )

> [spec:hfst:sem:operators.sfst.transducer.rec-cat-nodes-fn]
> Concatenates by redirecting every final node reachable from `node` to `node2`
> via an epsilon arc, making them non-final. If `node` has not been visited under
> `vmark` (the call marks it): first iterate over all outgoing arcs of `node` and
> recurse into each `arc->target_node()` (depth-first, so the original arc set is
> traversed before mutation). Then, if `node->is_final()`, clear its finality
> (`node->set_final(0)`) and add an epsilon arc (`Label()`) from `node` to
> `node2` (via `node->add_arc(Label(), node2, this)`). Returns nothing. Note the
> recursion happens before the finality edit, so newly added epsilon arcs to
> `node2` are not themselves recursed into.

> [spec:hfst:def:operators.sfst.transducer.recode-label-fn]
> Label Transducer::recode_label( Label l, bool lswitch, bool recode,

> [spec:hfst:sem:operators.sfst.transducer.recode-label-fn]
> Transforms label `l` according to two independent flags and returns the result.
> If `lswitch` is true, swap upper and lower: rebuild `l = Label(l.upper_char(),
> l.lower_char())` (the new lower becomes the old upper and vice versa). If
> `recode` is true, re-map the characters into the target alphabet `al`: look up
> each character's symbol string in `this`'s `alphabet` via `code2symbol` and add
> it to `al` via `add_symbol`, obtaining new codes — `lc` from the (current)
> lower char and `uc` from the (current) upper char — then set `l = Label(lc,
> uc)` and call `al.insert(l)` to register the label. (Note the recode step
> assigns the lower-derived code as the label's lower and the upper-derived code
> as the label's upper.) Returns the (possibly transformed) label `l`. If neither
> flag is set, returns `l` unchanged.

> [spec:hfst:def:operators.sfst.transducer.replace-char2-fn]
> void Transducer::replace_char2(Node *node, Node *node2, Character c,

> [spec:hfst:sem:operators.sfst.transducer.replace-char2-fn]
> Recursively copies the subgraph at `node` into transducer `a` (mirrored onto
> `node2`), replacing every occurrence of character `c` with `nc` in arc labels.
> Uses `forward` pointers + `vmark` for memoisation. If `node` has not been
> visited under `vmark`: pair via `node->set_forward(node2)` (marks visited). If
> `node->is_final()`, mark `node2` final. Then for each outgoing arc of `node`:
> let `t = arc->target_node()`; if `t->check_visited(vmark)` reuse
> `t2 = t->forward()`, else create `t2 = a->new_node()`. Add an arc from `node2`
> to `t2` with the transformed label `arc->label().replace_char(c, nc)`, then
> recurse `replace_char2(t, t2, c, nc, a)`. Returns nothing.

> [spec:hfst:def:operators.sfst.transducer.reverse-node-fn]
> void Transducer::reverse_node( Node *node, Transducer *na )

> [spec:hfst:sem:operators.sfst.transducer.reverse-node-fn]
> Recursively builds the reversal of the subgraph at `node` into transducer `na`.
> Uses `forward` pointers + `vmark` for memoisation. If `node` has not been
> visited under `vmark` (the call marks it): create a new node in `na` via
> `na->new_node()` and store it as `node->set_forward(...)`. If `node->is_final()`,
> add an epsilon arc (`Label()`) from `na`'s root node to this node's forward
> counterpart (final nodes become start points of the reversed automaton). Then
> for each outgoing arc of `node`: recurse `reverse_node(arc->target_node(), na)`
> to ensure the target's counterpart exists, let `n = arc->target_node()->forward()`,
> and add a reversed arc with the original `arc->label()` from `n` to
> `node->forward()` (edge direction flipped). Returns nothing. (The caller sets
> the forward of the original root as final after the traversal.)

> [spec:hfst:def:operators.sfst.transducer.splice-arc-fn]
> void Transducer::splice_arc( Node *node, Node *node2, Node *next_node,

> [spec:hfst:sem:operators.sfst.transducer.splice-arc-fn]
> Recursively copies the spliced transducer's subgraph rooted at `node` (a node
> of the inserted transducer `sa`) into `a`, starting from `node2`, and wires its
> accepting paths through to `next_node`. This does NOT use visited marks, so it
> fully unrolls the inserted transducer (assumed acyclic). If `node->is_final()`,
> add an epsilon arc (`Label()`) from `node2` to `next_node` (the continuation
> after the splice) and return. Otherwise, for each outgoing arc of `node`:
> create a fresh node `tn = a->new_node()`, add an arc with the same
> `arc->label()` from `node2` to `tn`, then recurse
> `splice_arc(arc->target_node(), tn, next_node, a)`. Returns nothing.

> [spec:hfst:def:operators.sfst.transducer.splice-nodes-fn]
> void Transducer::splice_nodes(Node *node, Node *node2, Label sl,

> [spec:hfst:sem:operators.sfst.transducer.splice-nodes-fn]
> Recursively copies the subgraph at `node` into result transducer `a` (mirrored
> onto `node2`), but wherever an arc's label equals the splice label `sl`, splices
> in a copy of transducer `sa` instead of a single arc. Uses `forward` pointers +
> `vmark` for memoisation. If `node` has not been visited under `vmark`: pair via
> `node->set_forward(node2)` (marks visited). If `node->is_final()`, mark `node2`
> final. Then for each outgoing arc of `node`: let `t = arc->target_node()`; if
> `t->check_visited(vmark)` reuse `t2 = t->forward()`, else create
> `t2 = a->new_node()`. If `arc->label() == sl`, splice the inserted transducer by
> calling `splice_arc(sa->root_node(), node2, t2, a)` (which builds `sa`'s graph
> from `node2` and connects its final paths to `t2`); otherwise add a plain arc
> with `arc->label()` from `node2` to `t2`. In either case, recurse
> `splice_nodes(t, t2, sl, sa, a)`. Returns nothing.

