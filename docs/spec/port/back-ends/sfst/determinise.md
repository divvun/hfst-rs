# back-ends/sfst/determinise.cc

> [spec:hfst:def:determinise.sfst.compute-transitions-fn]
> static void compute_transitions( NodeArray &na, vector<DTransition> &t )

> [spec:hfst:sem:determinise.sfst.compute-transitions-fn]
> Computes the outgoing deterministic transitions from a node set `na`,
> appending them to the output vector `t`.
> Creates a local `Label2NodeSet lmap` (a map from `Label` to `NodeSet`).
> For each node index `i` in `0..na.size()`, let `n = na[i]`. Iterate over
> `n`'s non-epsilon arcs (using `ArcsIter(n->arcs(), ArcsIter::non_eps)`):
> for each arc, look up/insert the entry `lmap[arc->label()]` and call its
> `add(arc->target_node())` (which also pulls in epsilon-reachable nodes).
> After collecting, reserve `lmap.size()` slots in `t`. Then iterate `lmap`
> in its (sorted-by-Label) order; for each entry `(label, nodeset)`, push a
> `DTransition(label, new NodeArray(nodeset))` onto `t`. Each `NodeArray` is
> heap-allocated; ownership passes to the caller via `t`. No return value.

> [spec:hfst:def:determinise.sfst.d-transition]
> class DTransition {
>   Label label;
>   NodeArray *nodes;
> }

> [spec:hfst:def:determinise.sfst.d-transition.d-transition-fn]
> DTransition(Label l, NodeArray *na)

> [spec:hfst:sem:determinise.sfst.d-transition.d-transition-fn]
> Constructor taking a `Label l` and a pointer `NodeArray *na`. Stores `l`
> into the member `label` and `na` into the member `nodes`. No ownership
> transfer logic beyond the pointer copy; no allocations.

> [spec:hfst:def:determinise.sfst.determinise-node-fn]
> static void determinise_node( NodeArray &na, Node *node, Transducer *a,

> [spec:hfst:sem:determinise.sfst.determinise-node-fn]
> Recursively builds the deterministic transducer for the subset-construction
> state `na`, which corresponds to the already-created destination `node` in
> transducer `a`, using `map` to track which `NodeArray` subsets have already
> been assigned a node.
> First set `node->set_final(na.is_final())`.
> Build a local `vector<DTransition> t` and fill it via
> `compute_transitions(na, t)`.
> Then for each `i` in `0..t.size()`:
> look up `it = map.find(t[i].nodes)`.
> If not found (`it == map.end()`): allocate a fresh node
> `target_node = a->new_node()`, register `map[t[i].nodes] = target_node`,
> add an arc from `node` labelled `t[i].label` to `target_node` via
> `node->add_arc(t[i].label, target_node, a)`, then recurse with
> `determinise_node(*t[i].nodes, target_node, a, map)`. Here the `NodeArray`
> ownership is retained by `map`.
> If found: the subset already has a node, so `delete t[i].nodes` (free the
> duplicate `NodeArray`) and add an arc from `node` labelled `t[i].label` to
> the existing `it->second` node. No return value.

> [spec:hfst:def:determinise.sfst.label2-node-set]
> class Label2NodeSet {
>   LabelMap lm;
> }

> [spec:hfst:def:determinise.sfst.label2-node-set.begin-fn]
> iterator begin()

> [spec:hfst:sem:determinise.sfst.label2-node-set.begin-fn]
> Returns `lm.begin()`, an iterator to the first `(Label, NodeSet)` entry of
> the underlying map (ordered by `Label`).

> [spec:hfst:def:determinise.sfst.label2-node-set.end-fn]
> iterator end()

> [spec:hfst:sem:determinise.sfst.label2-node-set.end-fn]
> Returns `lm.end()`, the past-the-end iterator of the underlying map.

> [spec:hfst:def:determinise.sfst.label2-node-set.find-fn]
> iterator find( Label l)

> [spec:hfst:sem:determinise.sfst.label2-node-set.find-fn]
> Returns `lm.find(l)`: an iterator to the entry whose key equals `Label l`,
> or `end()` if no such entry exists. Does not insert.

> [spec:hfst:def:determinise.sfst.label2-node-set.iterator]
> typedef LabelMap::iterator iterator

> [spec:hfst:def:determinise.sfst.label2-node-set.label-map]
> typedef map<const Label, NodeSet> LabelMap

> [spec:hfst:def:determinise.sfst.label2-node-set.label2-node-set-fn]
> Label2NodeSet(): lm()

> [spec:hfst:sem:determinise.sfst.label2-node-set.label2-node-set-fn]
> Default constructor; value-initializes the member map `lm` to empty. No
> other work.

> [spec:hfst:def:determinise.sfst.label2-node-set.size-fn]
> size_t size()

> [spec:hfst:sem:determinise.sfst.label2-node-set.size-fn]
> Returns `lm.size()`, the number of distinct `Label` keys currently in the
> map.

> [spec:hfst:def:determinise.sfst.node-array]
> class NodeArray {
>   size_t sizev;
>   bool final;
>   Node **node;
> }

> [spec:hfst:def:determinise.sfst.node-array.is-final-fn]
> bool is_final() const

> [spec:hfst:sem:determinise.sfst.node-array.is-final-fn]
> Returns the `final` member: true iff any node in the original node set was a
> final node (as computed during construction).

> [spec:hfst:def:determinise.sfst.node-array.node-array-fn]
> NodeArray::NodeArray( NodeSet &ns )

> [spec:hfst:sem:determinise.sfst.node-array.node-array-fn]
> Constructs a `NodeArray` from a `NodeSet &ns`. Initializes `sizev = 0` and
> `final = false`, and allocates `node = new Node*[ns.size()]` (capacity for
> all nodes in the set).
> Iterates over `ns` from `begin()` to `end()`. For each node `nn`:
> if `nn->arcs()->non_epsilon_transition_exists()` is true, store it into
> `node[sizev++]` (i.e. only nodes that have at least one non-epsilon outgoing
> arc are retained, and `sizev` counts those). Independently, if
> `nn->is_final()` is true, set `final = true`.
> Note the allocated array may be larger than the final `sizev`. The
> destructor frees the array with `delete[] node`.

> [spec:hfst:def:determinise.sfst.node-array.size-fn]
> size_t size() const

> [spec:hfst:sem:determinise.sfst.node-array.size-fn]
> Returns the `sizev` member: the count of retained nodes (those with a
> non-epsilon outgoing transition).

> [spec:hfst:def:determinise.sfst.node-mapping]
> class NodeMapping {
>   struct hashf { // [spec:hfst:def:determinise.sfst.node-mapping.hashf.operator-fn] // [spec:hfst:sem:determinise.sfst.node-mapping.hashf.operator-fn] size_t o...;
>   struct equalf { // [spec:hfst:def:determinise.sfst.node-mapping.equalf.operator-fn] // [spec:hfst:sem:determinise.sfst.node-mapping.equalf.operator-fn] int o...;
>   NodeMap hm;
> }

> [spec:hfst:def:determinise.sfst.node-mapping.begin-fn]
> iterator begin()

> [spec:hfst:sem:determinise.sfst.node-mapping.begin-fn]
> Returns `hm.begin()`, an iterator to the first `(NodeArray*, Node*)` entry
> of the underlying hash map.

> [spec:hfst:def:determinise.sfst.node-mapping.end-fn]
> iterator end()

> [spec:hfst:sem:determinise.sfst.node-mapping.end-fn]
> Returns `hm.end()`, the past-the-end iterator of the underlying hash map.

> [spec:hfst:def:determinise.sfst.node-mapping.equalf]
> struct equalf

> [spec:hfst:def:determinise.sfst.node-mapping.equalf.operator-fn]
> int operator()(const NodeArray *na1, const NodeArray *na2) const

> [spec:hfst:sem:determinise.sfst.node-mapping.equalf.operator-fn]
> Equality functor for the hash map keys. Given two `const NodeArray*` `na1`
> and `na2`, returns 0 (not equal) if their sizes differ or their `is_final()`
> flags differ. Otherwise iterates `i` in `0..na1->size()` and returns 0 as
> soon as `(*na1)[i] != (*na2)[i]` (pointer comparison of the i-th node). If
> all match, returns 1 (equal). Treats two node arrays as equal iff same
> length, same final flag, and identical node pointers in order.

> [spec:hfst:def:determinise.sfst.node-mapping.find-fn]
> iterator find( NodeArray *na)

> [spec:hfst:sem:determinise.sfst.node-mapping.find-fn]
> Returns `hm.find(na)`: an iterator to the entry whose `NodeArray*` key is
> equal to `na` (per the `hashf`/`equalf` functors), or `end()` if none.

> [spec:hfst:def:determinise.sfst.node-mapping.hashf]
> struct hashf

> [spec:hfst:def:determinise.sfst.node-mapping.hashf.operator-fn]
> size_t operator()(const NodeArray *na) const

> [spec:hfst:sem:determinise.sfst.node-mapping.hashf.operator-fn]
> Hash functor for the hash map keys. Given a `const NodeArray *na`, compute
> an initial `key = na->size() ^ na->is_final()` (XOR of size with the boolean
> final flag). Then for each `i` in `0..na->size()`, update
> `key = (key << 1) ^ (size_t)(*na)[i]` (XOR-fold the left-shifted running key
> with the i-th node pointer cast to `size_t`). Return the final `key`.

> [spec:hfst:def:determinise.sfst.node-mapping.iterator]
> typedef NodeMap::iterator iterator

> [spec:hfst:def:determinise.sfst.node-mapping.node-map]
> typedef hash_map<NodeArray*, Node*, hashf, equalf> NodeMap

> [spec:hfst:def:determinise.sfst.node-mapping.node-mapping-fn]
> NodeMapping::~NodeMapping()

> [spec:hfst:sem:determinise.sfst.node-mapping.node-mapping-fn]
> Destructor. Walks the hash map `hm` and frees every key `NodeArray`. Using
> an iterator `it` starting at `hm.begin()`: in each iteration capture
> `na = it->first`, save `old = it` then advance `it` (post-increment),
> `hm.erase(old)` to remove the entry, then `delete na`. Continues until
> `it == hm.end()`. This ensures the key `NodeArray` objects are deleted only
> after being erased from the map (avoiding a crash from the map holding
> dangling keys). Leaves the map empty.

> [spec:hfst:def:determinise.sfst.node-set]
> class NodeSet {
>   set<Node*> ht;
> }

> [spec:hfst:def:determinise.sfst.node-set.add-fn]
> void NodeSet::add( Node *node )

> [spec:hfst:sem:determinise.sfst.node-set.add-fn]
> Adds `node` to the set together with its epsilon-closure. Inserts `node`
> into the underlying `set<Node*> ht`. If the insertion actually added a new
> element (`result.second` is true): iterate `node`'s epsilon arcs via
> `ArcsIter(node->arcs(), ArcsIter::eps)`; for each arc, if
> `arc->label().is_epsilon()` is false, break out of the loop; otherwise
> recursively call `add(arc->target_node())`. If `node` was already present,
> do nothing further. Net effect: closes the set under epsilon transitions.

> [spec:hfst:def:determinise.sfst.node-set.begin-fn]
> iterator begin() const

> [spec:hfst:sem:determinise.sfst.node-set.begin-fn]
> Returns `ht.begin()`, an iterator to the first `Node*` in the underlying set
> (set order).

> [spec:hfst:def:determinise.sfst.node-set.clear-fn]
> void clear()

> [spec:hfst:sem:determinise.sfst.node-set.clear-fn]
> Calls `ht.clear()`, removing all nodes from the set. No return value.

> [spec:hfst:def:determinise.sfst.node-set.end-fn]
> iterator end() const

> [spec:hfst:sem:determinise.sfst.node-set.end-fn]
> Returns `ht.end()`, the past-the-end iterator of the underlying set.

> [spec:hfst:def:determinise.sfst.node-set.insert-fn]
> bool insert(Node *node)

> [spec:hfst:sem:determinise.sfst.node-set.insert-fn]
> Inserts `node` into the underlying `set<Node*> ht` via `ht.insert(node)`.
> Returns the boolean `result.second`: true if the node was newly added,
> false if it was already present. Unlike `add`, does NOT follow epsilon
> transitions.

> [spec:hfst:def:determinise.sfst.node-set.iterator]
> typedef set<Node*>::iterator iterator

> [spec:hfst:def:determinise.sfst.node-set.node-set-fn]
> NodeSet()

> [spec:hfst:sem:determinise.sfst.node-set.node-set-fn]
> Default constructor; leaves the underlying `set<Node*> ht` empty. No other
> work.

> [spec:hfst:def:determinise.sfst.node-set.size-fn]
> size_t size() const

> [spec:hfst:sem:determinise.sfst.node-set.size-fn]
> Returns `ht.size()`, the number of nodes currently in the set.

