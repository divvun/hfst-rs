# back-ends/sfst/fst.cc, back-ends/sfst/fst.h

> [spec:hfst:def:fst.output-type]
> typedef enum

> [spec:hfst:def:fst.sfst.arc]
> class Arc {
>   Label l;
>   Node *target;
>   Arc *next;
> }

> [spec:hfst:def:fst.sfst.arc.init-fn]
> void init( Label ll, Node *node )

> [spec:hfst:sem:fst.sfst.arc.init-fn]
> Sets the Arc's label field `l` to the passed-in label `ll` and its
> `target` pointer to the passed-in node pointer `node`. Does not touch
> the `next` field. No return value.

> [spec:hfst:def:fst.sfst.arc.label-fn]
> Label label( void ) const

> [spec:hfst:sem:fst.sfst.arc.label-fn]
> Const accessor: returns the Arc's stored Label `l` by value.

> [spec:hfst:def:fst.sfst.arc.target-node-fn]
> Node *target_node( void )

> [spec:hfst:sem:fst.sfst.arc.target-node-fn]
> Accessor: returns the Arc's stored `target` node pointer. There are two
> overloads, a non-const one returning `Node*` and a const one returning
> `const Node*`; both simply return the `target` field.

> [spec:hfst:def:fst.sfst.arcs]
> class Arcs {
>   Arc *first_arcp;
>   Arc *first_epsilon_arcp;
> }

> [spec:hfst:def:fst.sfst.arcs-iter]
> class ArcsIter {
>   Arc *current_arcp;
>   Arc *more_arcs;
> }

> [spec:hfst:def:fst.sfst.arcs-iter.arcs-iter-fn]
> ArcsIter( const Arcs *arcs, IterType type=all )

> [spec:hfst:sem:fst.sfst.arcs-iter.arcs-iter-fn]
> Constructs an iterator over the arcs of `arcs`, ordering epsilon arcs
> before non-epsilon arcs. Sets `more_arcs` to NULL initially. If `type`
> is `all`: if `arcs->first_epsilon_arcp` is non-NULL, set `current_arcp`
> to that epsilon-arc head and set `more_arcs` to `arcs->first_arcp` (so
> the non-epsilon chain is visited after the epsilon chain); otherwise set
> `current_arcp` to `arcs->first_arcp`. If `type` is `non_eps`: set
> `current_arcp` to `arcs->first_arcp`. Otherwise (`eps`): set
> `current_arcp` to `arcs->first_epsilon_arcp`. `type` defaults to `all`.

> [spec:hfst:def:fst.sfst.arcs-iter.iter-type]
> typedef enum

> [spec:hfst:def:fst.sfst.arcs-iter.operator-fn]
> void operator++( int )

> [spec:hfst:sem:fst.sfst.arcs-iter.operator-fn]
> Post-increment advance. If `current_arcp` is non-NULL, advance it to
> `current_arcp->next`. If that becomes NULL but `more_arcs` is non-NULL,
> set `current_arcp` to `more_arcs` (the non-epsilon chain saved by the
> `all` constructor) and clear `more_arcs` to NULL. If `current_arcp` was
> already NULL, do nothing. No return value. (The companion conversion
> operator `operator Arc*()` returns `current_arcp`, used to test/extract
> the current arc.)

> [spec:hfst:def:fst.sfst.arcs.add-arc-fn]
> void Arcs::add_arc( Label l, Node *node, Transducer *a )

> [spec:hfst:sem:fst.sfst.arcs.add-arc-fn]
> Allocates a new Arc via `a->new_arc(l, node)` (using transducer `a`'s
> memory pool). If the label `l` is epsilon (`l.is_epsilon()`), prepends
> the arc to the epsilon list: set `arc->next = first_epsilon_arcp` then
> `first_epsilon_arcp = arc`. Otherwise prepends it to the non-epsilon
> list: set `arc->next = first_arcp` then `first_arcp = arc`. No return
> value. New arcs are inserted at the head of their respective list.

> [spec:hfst:def:fst.sfst.arcs.arcs-fn]
> Arcs( void )

> [spec:hfst:sem:fst.sfst.arcs.arcs-fn]
> Default constructor: calls `init()`, which sets both `first_arcp` and
> `first_epsilon_arcp` to NULL (empty arc lists).

> [spec:hfst:def:fst.sfst.arcs.epsilon-transition-exists-fn]
> bool epsilon_transition_exists( void ) const

> [spec:hfst:sem:fst.sfst.arcs.epsilon-transition-exists-fn]
> Const predicate: returns true iff `first_epsilon_arcp != NULL`, i.e. at
> least one epsilon arc exists.

> [spec:hfst:def:fst.sfst.arcs.init-fn]
> void init( void )

> [spec:hfst:sem:fst.sfst.arcs.init-fn]
> Sets both `first_arcp` and `first_epsilon_arcp` to NULL, leaving the
> Arcs object with empty non-epsilon and epsilon arc lists. No return
> value.

> [spec:hfst:def:fst.sfst.arcs.is-empty-fn]
> bool is_empty( void ) const

> [spec:hfst:sem:fst.sfst.arcs.is-empty-fn]
> Const predicate: returns `!(first_arcp || first_epsilon_arcp)`, i.e.
> true iff both arc lists are empty (no non-epsilon and no epsilon arcs).

> [spec:hfst:def:fst.sfst.arcs.non-epsilon-transition-exists-fn]
> bool non_epsilon_transition_exists( void ) const

> [spec:hfst:sem:fst.sfst.arcs.non-epsilon-transition-exists-fn]
> Const predicate: returns true iff `first_arcp != NULL`, i.e. at least
> one non-epsilon arc exists.

> [spec:hfst:def:fst.sfst.arcs.remove-arc-fn]
> int Arcs::remove_arc( Arc *arc )

> [spec:hfst:sem:fst.sfst.arcs.remove-arc-fn]
> Removes the given `arc` from whichever singly-linked list it belongs to.
> Selects the list head pointer-to-pointer `p`: `&first_epsilon_arcp` if
> `arc->label().is_epsilon()`, else `&first_arcp`. Walks the chain
> (`p = &(*p)->next`) until `*p` equals `arc`; when found, splices it out
> by setting `*p = arc->next` and returns 1. If the arc is not found in
> the list, returns 0. The Arc object itself is not freed.

> [spec:hfst:def:fst.sfst.arcs.size-fn]
> int Arcs::size() const

> [spec:hfst:sem:fst.sfst.arcs.size-fn]
> Const. Counts the total number of arcs. Initializes a counter `n` to 0,
> walks the non-epsilon list from `first_arcp` following `->next`
> incrementing `n` for each, then walks the epsilon list from
> `first_epsilon_arcp` likewise. Returns `n` (the sum of both list
> lengths).

> [spec:hfst:def:fst.sfst.arcs.target-node-fn]
> Node *Arcs::target_node( Label l )

> [spec:hfst:sem:fst.sfst.arcs.target-node-fn]
> Finds the target node reached by the arc whose label equals `l`. Walks
> only the non-epsilon list from `first_arcp` following `->next`; for each
> arc, if `arc->label() == l`, returns `arc->target_node()`. If no match
> is found, returns NULL. (The epsilon list is not searched.) There are
> two overloads, a non-const returning `Node*` and a const returning
> `const Node*`, with identical logic.

> [spec:hfst:def:fst.sfst.complete-fn]
> static void complete( Node *node, Alphabet &alphabet, VType vmark)

> [spec:hfst:sem:fst.sfst.complete-fn]
> Free (static) recursive function. Visits the subgraph rooted at `node`,
> inserting every non-epsilon arc label into `alphabet`. First calls
> `node->was_visited(vmark)`: if it returns true (node already marked with
> `vmark`), return immediately; otherwise it marks the node as visited.
> Then iterates over all arcs of the node (epsilon first, via ArcsIter):
> for each arc, if its label is not epsilon, calls
> `alphabet.insert(arc->label())`; then recurses into
> `complete(arc->target_node(), alphabet, vmark)`. No return value.

> [spec:hfst:def:fst.sfst.error-message-fn]
> static void error_message( size_t line )

> [spec:hfst:sem:fst.sfst.error-message-fn]
> Free (static) function that always throws. Formats into a static 1000-
> byte buffer `message` the string
> `"Error: in line %u of text transducer file"` with `line` (cast to
> `unsigned int`) substituted, then throws that `char*` buffer as a C++
> exception. Never returns normally.

> [spec:hfst:def:fst.sfst.hashf]
> struct hashf

> [spec:hfst:def:fst.sfst.hashf.operator-fn]
> size_t operator()(const Node *n) const

> [spec:hfst:sem:fst.sfst.hashf.operator-fn]
> Hash functor for `const Node*`: returns the pointer value reinterpreted
> as `size_t` (`(size_t) n`), i.e. identity hash on the node's address.

> [spec:hfst:def:fst.sfst.index]
> typedef unsigned Index

> [spec:hfst:def:fst.sfst.next-string-fn]
> static char *next_string( char* &s, size_t line )

> [spec:hfst:sem:fst.sfst.next-string-fn]
> Free (static) function that extracts and unquotes the next field from a
> tab/newline-delimited text line, editing the buffer in place. Takes
> `s` by reference (a `char*` cursor) and a `line` number for error
> reporting. Uses two cursors `p` and `q` both starting at `s`. Scans `q`
> forward while the char is not NUL, tab, '\n', or '\r': if `*q` is a
> backslash, advance `q` once (so the following char is taken literally),
> then copy `*(p++) = *(q++)`. This compacts/unquotes the field into the
> start of the buffer. If nothing was consumed (`p == s`), calls
> `error_message(line)` (which throws). Records `result = s` (start of the
> field). Then skips trailing whitespace by advancing `q` over spaces,
> tabs, '\n', '\r'. If `*q == 0` (end of line reached), sets `s = NULL`;
> otherwise sets `s = q` to point at the next field. Writes a NUL at `*p`
> to terminate the result string. Returns `result`.

> [spec:hfst:def:fst.sfst.node]
> class Node {
>   Arcs arcsp;
>   Node *forwardp;
>   VType visited;
>   bool final;
>   Index index;
> }

> [spec:hfst:def:fst.sfst.node-hash-set]
> typedef hash_set<const Node*, hashf> NodeHashSet

> [spec:hfst:def:fst.sfst.node-in-copy-tr-fn]
> Node *node_in_copy_tr( Node *node, Transducer *copy_tr, map<int, Node*> &mapper )

> [spec:hfst:sem:fst.sfst.node-in-copy-tr-fn]
> Free function. Finds (or lazily creates) the node in `copy_tr` that
> corresponds to `node` from the original transducer, using `mapper`
> (a `map<int,Node*>` keyed by the original node's `index`). Reads
> `node_index = (int)node->index`. Looks it up in `mapper`. If absent:
> creates a new node via `copy_tr->new_node()`; if the original `node` is
> final, marks the new node final via `set_final(true)`; stores
> `mapper[node_index] = associated_node`; returns the new node. If present:
> returns the already-mapped node (`it->second`).

> [spec:hfst:def:fst.sfst.node.add-arc-fn]
> void add_arc( Label l, Node *n, Transducer *a )

> [spec:hfst:sem:fst.sfst.node.add-arc-fn]
> Delegates to this node's Arcs object: calls `arcs()->add_arc(l, n, a)`,
> adding an arc with label `l` to target node `n`, allocated via
> transducer `a`. No return value.

> [spec:hfst:def:fst.sfst.node.arcs-fn]
> Arcs *arcs( void )

> [spec:hfst:sem:fst.sfst.node.arcs-fn]
> Accessor: returns a pointer to this node's embedded Arcs member,
> `&arcsp`. Two overloads: non-const returns `Arcs*`, const returns
> `const Arcs*`.

> [spec:hfst:def:fst.sfst.node.check-visited-fn]
> bool check_visited( VType vm ) // leaves the visited flag unchanged

> [spec:hfst:sem:fst.sfst.node.check-visited-fn]
> Returns `visited == vm` (true iff this node's `visited` marker already
> equals `vm`). Unlike `was_visited`, this leaves the `visited` field
> unchanged.

> [spec:hfst:def:fst.sfst.node.clear-visited-fn]
> void Node::clear_visited( NodeHashSet &nodeset )

> [spec:hfst:sem:fst.sfst.node.clear-visited-fn]
> Recursively resets the `visited` marker to 0 across the subgraph reached
> from this node, using `nodeset` (a NodeHashSet) to avoid revisiting.
> If `this` is not already in `nodeset` (find returns end()): set
> `visited = 0`; insert `this` into `nodeset`; print to stderr a space
> followed by the current `nodeset.size()` (as unsigned long, format
> `" %lu"`); then iterate over all arcs of this node (via ArcsIter) and
> for each call `arc->target_node()->clear_visited(nodeset)` recursively.
> If `this` is already in `nodeset`, do nothing. No return value.

> [spec:hfst:def:fst.sfst.node.forward-fn]
> Node *forward( void )

> [spec:hfst:sem:fst.sfst.node.forward-fn]
> Accessor: returns the node's `forwardp` pointer (the forward/auxiliary
> node pointer used as a flag during algorithms such as epsilon removal).

> [spec:hfst:def:fst.sfst.node.init-fn]
> void Node::init()

> [spec:hfst:sem:fst.sfst.node.init-fn]
> Initializes the node to its default empty state: sets `final = false`,
> `visited = 0`, calls `arcsp.init()` (clearing both arc-list heads to
> NULL), and sets `forwardp = NULL`. The `index` field is not touched.
> No return value.

> [spec:hfst:def:fst.sfst.node.is-final-fn]
> bool is_final( void ) const

> [spec:hfst:sem:fst.sfst.node.is-final-fn]
> Const accessor: returns the node's `final` boolean flag.

> [spec:hfst:def:fst.sfst.node.node-fn]
> Node( void )

> [spec:hfst:sem:fst.sfst.node.node-fn]
> Default constructor: calls `init()`, setting `final = false`,
> `visited = 0`, clearing the arc lists, and `forwardp = NULL`.

> [spec:hfst:def:fst.sfst.node.set-final-fn]
> void set_final( bool flag )

> [spec:hfst:sem:fst.sfst.node.set-final-fn]
> Sets the node's `final` boolean flag to the passed-in `flag`. No return
> value.

> [spec:hfst:def:fst.sfst.node.set-forward-fn]
> void set_forward( Node *node )

> [spec:hfst:sem:fst.sfst.node.set-forward-fn]
> Sets the node's `forwardp` pointer to the passed-in `node`. No return
> value.

> [spec:hfst:def:fst.sfst.node.target-node-fn]
> const Node *target_node( Label l ) const

> [spec:hfst:sem:fst.sfst.node.target-node-fn]
> Delegates to the node's Arcs object: returns
> `arcs()->target_node(l)`, i.e. the target node reached by the
> non-epsilon arc whose label equals `l`, or NULL if none. Two overloads,
> const returning `const Node*` and non-const returning `Node*`.

> [spec:hfst:def:fst.sfst.node.was-visited-fn]
> bool was_visited( VType vmark )

> [spec:hfst:sem:fst.sfst.node.was-visited-fn]
> Test-and-set on the node's `visited` marker. If `visited == vmark`,
> return true (already visited this pass). Otherwise set `visited = vmark`
> and return false. Used to detect/mark first visits during traversals.

> [spec:hfst:def:fst.sfst.pair-mapping]
> class PairMapping {
>   struct hashf { // [spec:hfst:def:fst.sfst.pair-mapping.hashf.operator-fn] // [spec:hfst:sem:fst.sfst.pair-mapping.hashf.operator-fn] size_t operator()(const ...;
>   struct equalf { // [spec:hfst:def:fst.sfst.pair-mapping.equalf.operator-fn] // [spec:hfst:sem:fst.sfst.pair-mapping.equalf.operator-fn] int operator()(const ...;
>   PairMap pm;
> }

> [spec:hfst:def:fst.sfst.pair-mapping.begin-fn]
> iterator begin( void )

> [spec:hfst:sem:fst.sfst.pair-mapping.begin-fn]
> Returns `pm.begin()`, an iterator to the first entry of the underlying
> hash_map `pm` (delegates directly to the map's `begin`).

> [spec:hfst:def:fst.sfst.pair-mapping.end-fn]
> iterator end( void )

> [spec:hfst:sem:fst.sfst.pair-mapping.end-fn]
> Returns `pm.end()`, the past-the-end iterator of the underlying
> hash_map `pm` (delegates directly to the map's `end`).

> [spec:hfst:def:fst.sfst.pair-mapping.equalf]
> struct equalf

> [spec:hfst:def:fst.sfst.pair-mapping.equalf.operator-fn]
> int operator()(const NodePair p1, const NodePair p2) const

> [spec:hfst:sem:fst.sfst.pair-mapping.equalf.operator-fn]
> Equality functor for two `NodePair`s: returns (as int) true iff both
> components match, i.e. `p1.first == p2.first && p1.second == p2.second`.

> [spec:hfst:def:fst.sfst.pair-mapping.find-fn]
> iterator find( Node *n1, Node *n2 )

> [spec:hfst:sem:fst.sfst.pair-mapping.find-fn]
> Looks up the node pair `(n1, n2)` in the map: constructs a
> `NodePair(n1, n2)` and returns `pm.find(...)`, i.e. an iterator to the
> matching entry or `pm.end()` if the pair is not present.

> [spec:hfst:def:fst.sfst.pair-mapping.hashf]
> struct hashf

> [spec:hfst:def:fst.sfst.pair-mapping.hashf.operator-fn]
> size_t operator()(const NodePair p) const

> [spec:hfst:sem:fst.sfst.pair-mapping.hashf.operator-fn]
> Hash functor for a `NodePair`: returns the bitwise XOR of the two node
> pointers reinterpreted as `size_t`, i.e.
> `(size_t)p.first ^ (size_t)p.second`.

> [spec:hfst:def:fst.sfst.pair-mapping.iterator]
> typedef PairMap::iterator iterator

> [spec:hfst:def:fst.sfst.pair-mapping.node-pair]
> typedef std::pair<Node*, Node*> NodePair

> [spec:hfst:def:fst.sfst.pair-mapping.pair-map]
> typedef hash_map<NodePair, Node*, hashf, equalf> PairMap

> [spec:hfst:def:fst.sfst.print-node-fn]
> static void print_node( ostream &s, Node *node, VType vmark, Alphabet &abc )

> [spec:hfst:sem:fst.sfst.print-node-fn]
> Free (static) recursive function that prints the subgraph rooted at
> `node` to ostream `s` in AT&T-style text format. Calls
> `node->was_visited(vmark)`: if it returns true (already visited),
> returns and does nothing. Otherwise (now marked) gets `arcs =
> node->arcs()` and iterates over all arcs (epsilon first, via ArcsIter):
> for each arc, writes one line `node->index "\t" target->index "\t"
> abc.write_char(label.lower_char()) "\t" abc.write_char(label.upper_char())
> "\n"`. After the arc lines, if `node->is_final()` writes a line with just
> `node->index "\n"`. Then iterates the arcs again and recurses
> `print_node(s, arc->target_node(), vmark, abc)` into each target. No
> return value. Relies on node `index` fields having been assigned (e.g.
> by nodeindexing) and `vmark` having been freshly incremented by the
> caller.

> [spec:hfst:def:fst.sfst.read-node-fn]
> static void read_node( FILE *file, Node *node, Node **p, Transducer *a )

> [spec:hfst:sem:fst.sfst.read-node-fn]
> Free (static) recursive function that reads one node and its subtree
> from binary file `file` into `node`, using index-to-node table `p` and
> transducer `a` for allocation. Reads one `char c` (the final flag) via
> fread; if fread does not return 1 throws "read_node: fread failed".
> Calls `node->set_final(c)`. Reads an `unsigned short n` (arc count) via
> fread; throws on short read. Loops `i` from 0 to `n-1`: reads three
> values via separate freads — `Character lc`, `Character uc`, and
> `unsigned int t` (target index) — each throwing "read_node: fread
> failed" on a short read; after reading, if `ferror(file)` throws "Error
> encountered while reading transducer from file". Then: if `p[t]` is
> non-NULL (target node already created), calls
> `node->add_arc(Label(lc,uc), p[t], a)`. Otherwise creates the target via
> `p[t] = a->new_node()`, calls `node->add_arc(Label(lc,uc), p[t], a)`,
> and recurses `read_node(file, p[t], p, a)` to read that new node's
> subtree. No return value.

> [spec:hfst:def:fst.sfst.store-arc-label-fn]
> static void store_arc_label( FILE *file, Arc *arc )

> [spec:hfst:sem:fst.sfst.store-arc-label-fn]
> Free (static) function that writes an arc's label to binary file `file`.
> Reads `l = arc->label()`, extracts `lc = l.lower_char()` and
> `uc = l.upper_char()` (both `Character`), then fwrites `lc` followed by
> `uc`, each as one element of its size. No return value.

> [spec:hfst:def:fst.sfst.store-lowmem-node-fn]
> static void store_lowmem_node( FILE *file, Node *node,

> [spec:hfst:sem:fst.sfst.store-lowmem-node-fn]
> Free (static) function that writes a single node (non-recursively) to
> binary file `file` in the low-memory format. `startpos` is a
> `vector<unsigned int>` mapping each node index to its byte start
> position in the file. Calls `store_node_info(file, node)` to write the
> final flag and arc count. Then iterates over all arcs of the node
> (epsilon first, via ArcsIter): for each arc calls
> `store_arc_label(file, arc)` (writes lower/upper Characters), then looks
> up `t = startpos[arc->target_node()->index]` and fwrites that
> `unsigned int` file offset (instead of a node index). No recursion, no
> return value.

> [spec:hfst:def:fst.sfst.store-node-fn]
> static void store_node( FILE *file, Node *node, VType vmark )

> [spec:hfst:sem:fst.sfst.store-node-fn]
> Free (static) recursive function that writes the subgraph rooted at
> `node` to binary file `file`. Calls `node->was_visited(vmark)`: if true
> (already written), returns and does nothing. Otherwise (now marked):
> calls `store_node_info(file, node)` (writes final flag and arc count),
> then iterates over all arcs (epsilon first, via ArcsIter): for each arc
> calls `store_arc_label(file, arc)`, then fwrites
> `t = (unsigned int)arc->target_node()->index` as an `unsigned int`, then
> recurses `store_node(file, arc->target_node(), vmark)`. No return value.
> Node `index` fields and a fresh `vmark` must be set up by the caller.

> [spec:hfst:def:fst.sfst.store-node-info-fn]
> static void store_node_info( FILE *file, Node *node )

> [spec:hfst:sem:fst.sfst.store-node-info-fn]
> Free (static) function that writes a node's header to binary file
> `file`. First writes the final flag: `char c = node->is_final()` then
> fwrites `c`. Then computes `nn = node->arcs()->size()` (the total arc
> count); if `nn > 65535` throws "Error: in function store_node\n".
> Otherwise casts to `unsigned short n` and fwrites `n`. No return value.

> [spec:hfst:def:fst.sfst.transducer]
> class Transducer {
>   VType vmark;
>   Node root;
>   Mem mem;
>   size_t node_count;
>   size_t transition_count;
>   static bool hopcroft_minimisation;
>   bool deterministic;
>   bool minimised;
>   bool indexed;
>   Alphabet alphabet;
>   Transducer &expand( set<char*> &s );
>   Transducer &remove_epsilons();
>   Transducer &copy( bool lswitch=false, const Alphabet *al=NULL );
>   Transducer &splice( Label l, Transducer *a);
>   Transducer &freely_insert( Label l );
>   Transducer &replace_char( Character c, Character nc );
>   Transducer &level( Level );
>   Transducer &determinise( bool copy_alphabet=true );
>   Transducer &rev_det_minimise( bool verbose );
>   Transducer &hopcroft_minimise( bool verbose );
>   Transducer &reverse( bool copy_alphabet=true );
>   Transducer &operator|( Transducer& );
>   Transducer &operator+( Transducer& );
>   Transducer &operator/( Transducer& );
>   Transducer &operator&( Transducer& );
>   Transducer &operator||( Transducer& );
>   Transducer &operator!( void );
>   Transducer &kleene_star( void );
> }

> [spec:hfst:def:fst.sfst.transducer.add-string-fn]
> void Transducer::add_string( char *s, bool extended, Alphabet *a )

> [spec:hfst:sem:fst.sfst.transducer.add-string-fn]
> Adds the string `s` as an accepting path into this transducer. If `a`
> (the Alphabet to tokenise with) is NULL, sets `a = &alphabet` (this
> transducer's own alphabet). Starts at `node = root_node()`. Loops:
> calls `l = a->next_label(s, extended)` (which consumes the next label
> from `s`); while `l` is not epsilon: inserts `l` into `a`
> (`a->insert(l)`); gets `arcs = node->arcs()` and follows the existing
> non-epsilon arc labelled `l` via `node = arcs->target_node(l)`; if that
> returns NULL, allocates a fresh node `node = new_node()` and adds an arc
> `arcs->add_arc(l, node, this)` to it. After the loop ends (epsilon
> reached, i.e. end of string), calls `node->set_final(1)` to mark the
> final node accepting. No return value.

> [spec:hfst:def:fst.sfst.transducer.analyze-string-fn]
> bool Transducer::analyze_string( char *string, FILE *file, bool with_brackets )

> [spec:hfst:sem:fst.sfst.transducer.analyze-string-fn]
> Analyzes the surface string `string` against this transducer and prints
> the resulting analyses to `file`. Tokenises `string` into a
> `vector<Character> input` via `alphabet.string2symseq(string, input)`,
> then builds `vector<Label> labels` where each label is `Label(input[i])`
> (an identity label per character). Constructs `Transducer a1(labels)`
> (the linear transducer for the input). Composes `a2 = &(*this || a1)`
> (this transducer composed with a1). Takes the lower projection
> `a3 = &(a2->lower_level())`, deletes `a2`, then minimises
> `a2 = &a3->minimise()` and deletes `a3`. Copies this transducer's
> alphabet into `a2` (`a2->alphabet.copy(alphabet)`). Calls
> `result = a2->print_strings(file, with_brackets)` to enumerate and print
> the analyses, deletes `a2`, and returns `result`. (Returns true if the
> result transducer was cyclic/too long per print_strings semantics.)

> [spec:hfst:def:fst.sfst.transducer.build-transtab-fn]
> void build_transtab( vector<Transition> &transtab )

> [spec:hfst:sem:fst.sfst.transducer.build-transtab-fn]
> Member function declared in fst.h (`void build_transtab(vector<Transition>
> &transtab)`) but with no definition anywhere in the SFST back-end or
> libhfst sources. It is never defined and never called; no behaviour to
> port. (A placeholder/dead declaration.)

> [spec:hfst:def:fst.sfst.transducer.build-tt-fn]
> void build_TT( Node *node, vector<Transition> &transtab )

> [spec:hfst:sem:fst.sfst.transducer.build-tt-fn]
> Member function declared in fst.h (`void build_TT(Node *node,
> vector<Transition> &transtab)`) but with no definition anywhere in the
> SFST back-end or libhfst sources. It is never defined and never called;
> no behaviour to port. (A placeholder/dead declaration.)

> [spec:hfst:def:fst.sfst.transducer.clear-fn]
> void Transducer::clear()

> [spec:hfst:sem:fst.sfst.transducer.clear-fn]
> Resets this transducer to an empty state (equivalent to a freshly
> default-constructed Transducer). Sets `vmark = 0`; sets `deterministic`
> and `minimised` both to false; calls `root.init()` to reset the root
> node (final=false, visited=0, empty arc lists, forwardp=NULL); calls
> `mem.clear()` to release the node/arc memory pool; and calls
> `alphabet.clear()` to empty the alphabet. No return value.

> [spec:hfst:def:fst.sfst.transducer.compare-nodes-fn]
> bool compare_nodes( Node *node, Node *node2, Transducer &a2 )

> [spec:hfst:sem:fst.sfst.transducer.compare-nodes-fn]
> Recursively tests whether the deterministic subgraph rooted at `node`
> (this transducer) is equivalent to that rooted at `node2` (transducer
> `a2`), using each transducer's `forward` pointer to record the pairing
> and `was_visited` to detect cycles. Steps: if
> `node->was_visited(vmark)` is true, then if `node2->was_visited(a2.vmark)`
> is also true return whether the recorded pairing is consistent
> (`node->forward() == node2 && node2->forward() == node`), else return
> false; if instead only `node2->was_visited(a2.vmark)` is true (node not
> visited) return false. Once neither was visited (both now marked), record
> the pairing: `node->set_forward(node2)` and `node2->set_forward(node)`.
> If `node->is_final() != node2->is_final()` return false. Iterate over all
> arcs of `node`: for each arc, find `t2 = node2->target_node(arc->label())`;
> if `t2 == NULL` return false, else recurse `compare_nodes(arc->target_node(),
> t2, a2)` and return false if it returns false. Then iterate over all arcs
> of `node2`: for each, if `node->target_node(arc->label()) == NULL` return
> false (ensures no extra arcs on node2's side). If all checks pass return
> true. Assumes both transducers are deterministic and that `vmark`/`a2.vmark`
> were freshly incremented by the caller.

> [spec:hfst:def:fst.sfst.transducer.complete-alphabet-fn]
> void Transducer::complete_alphabet()

> [spec:hfst:sem:fst.sfst.transducer.complete-alphabet-fn]
> Ensures the transducer's `alphabet` contains every non-epsilon label
> actually used on its arcs. Calls `incr_vmark()` to obtain a fresh visit
> marker, then calls the free `complete(root_node(), alphabet, vmark)`
> which recursively walks the whole graph from the root inserting each
> non-epsilon arc label into `alphabet`. No return value.

> [spec:hfst:def:fst.sfst.transducer.copy-nodes-fn]
> void Transducer::copy_nodes( Node *search_node, Transducer *copy_tr,

> [spec:hfst:sem:fst.sfst.transducer.copy-nodes-fn]
> Recursive epsilon-removal copy. Copies the arcs reachable from
> `search_node` (in this transducer) into `copy_tr`, attaching them to
> `copy_tr_start_node`, while collapsing epsilon transitions. `mapper`
> (a `map<int,Node*>`) associates original node indices with their copies
> in `copy_tr`; the `forward` pointer is used as an epsilon-loop flag.
> Iterates over all arcs of `search_node` (epsilon arcs first, via
> ArcsIter), taking `arc = *it` by value. If `arc.label().is_epsilon()`:
> only descend if `search_node->forward() != copy_tr_start_node` (not
> already in this epsilon closure) — set `search_node->set_forward(copy_tr_start_node)`
> as the loop flag; if `arc.target_node()->is_final()` mark
> `copy_tr_start_node->set_final(true)`; recurse
> `copy_nodes(arc.target_node(), copy_tr, copy_tr_start_node, mapper)`
> (still attaching to the same start node, since epsilon adds no symbol);
> then clear the flag `search_node->set_forward(NULL)`. Else (non-epsilon
> arc): find/create the copy target via
> `copy_tr_end_node = node_in_copy_tr(arc.target_node(), copy_tr, mapper)`,
> add an arc in copy_tr `copy_tr_start_node->add_arc(Label(lower_char,
> upper_char), copy_tr_end_node, copy_tr)`, and if the original target was
> not yet visited (`!arc.target_node()->was_visited(vmark)`) recurse
> `copy_nodes(arc.target_node(), copy_tr, copy_tr_end_node, mapper)`. No
> return value.

> [spec:hfst:def:fst.sfst.transducer.create-node-fn]
> Node *Transducer::create_node( vector<Node*> &node, char *s, size_t line )

> [spec:hfst:sem:fst.sfst.transducer.create-node-fn]
> Parses a node number from string `s` and returns the corresponding node,
> creating it if needed, using `node` (a `vector<Node*>` indexed by node
> number) as the index-to-node map; `line` is for error reporting. Parses
> `n = strtol(s, &p, 10)`. If no digits were consumed (`s == p`) or `n < 0`,
> calls `error_message(line)` (throws). If `node.size() <= n`, resizes the
> vector to `n+1` filling new slots with NULL. If `node[n]` is NULL,
> allocates `node[n] = new_node()`. Returns `node[n]`.

> [spec:hfst:def:fst.sfst.transducer.enumerate-paths-fn]
> bool Transducer::enumerate_paths( vector<Transducer*> &result )

> [spec:hfst:sem:fst.sfst.transducer.enumerate-paths-fn]
> Enumerates every accepting path of this transducer as a separate
> single-path Transducer, appended to `result`. First, if
> `is_infinitely_ambiguous()` returns true, returns true immediately
> (cannot enumerate an infinite set; `result` left untouched). Otherwise
> frees any pre-existing transducers in `result` (`delete result[i]` for
> all i) and clears it. Sets up an empty `vector<Label> path` and an empty
> `NodeHashSet previous`, then calls
> `enumerate_paths_node(root_node(), path, previous, result)` to do the
> recursive DFS that pushes one new Transducer(path) per accepting node.
> Returns false (finite enumeration completed).

> [spec:hfst:def:fst.sfst.transducer.enumerate-paths-node-fn]
> void Transducer::enumerate_paths_node( Node *node, vector<Label> &path,

> [spec:hfst:sem:fst.sfst.transducer.enumerate-paths-node-fn]
> Recursive DFS that collects every accepting path from `node` into
> `result`. `path` is the current label sequence, `previous` a NodeHashSet
> of nodes on the current path (cycle bookkeeping). If `node->is_final()`,
> pushes a new `Transducer(path)` (the linear transducer for the current
> label sequence) onto `result`. Then iterates over all arcs of `node`
> (epsilon first, via ArcsIter): for each arc, inserts `node` into
> `previous` (saving the returned iterator `it_`), pushes `arc->label()`
> onto `path`, recurses
> `enumerate_paths_node(arc->target_node(), path, previous, result)`, then
> pops the label off `path` and erases `it_` from `previous` (backtracks).
> No return value. (Note: although `previous` is maintained, this routine
> does not itself test it to prune cycles — infinite ambiguity is screened
> out by the caller `enumerate_paths` before invocation.)

> [spec:hfst:def:fst.sfst.transducer.expand-node-fn]
> void expand_node( Node *origin, Label &l, Node *target, Transducer *a, set<char*> &s )

> [spec:hfst:sem:fst.sfst.transducer.expand-node-fn]
> Member function declared in fst.h (`void expand_node(Node *origin, Label
> &l, Node *target, Transducer *a, set<char*> &s)`) but with no definition
> in the SFST back-end or libhfst sources for the SFST `Transducer` class.
> It is never defined and never called; no behaviour to port. (A
> placeholder/dead declaration; the analogous logic lives in
> `SfstTransducer::expand_node` in libhfst, a different class.)

> [spec:hfst:def:fst.sfst.transducer.expand-nodes-fn]
> Node *expand_nodes( Node *node, Transducer *a, set<char*> &s )

> [spec:hfst:sem:fst.sfst.transducer.expand-nodes-fn]
> Member function declared in fst.h (`Node *expand_nodes(Node *node,
> Transducer *a, set<char*> &s)`) but with no definition in the SFST
> back-end or libhfst sources for the SFST `Transducer` class. It is never
> defined and never called; no behaviour to port. (A placeholder/dead
> declaration; the analogous logic lives in `SfstTransducer::expand` in
> libhfst, a different class.)

> [spec:hfst:def:fst.sfst.transducer.freely-insert-at-node-fn]
> void freely_insert_at_node( Node *node, Label l )

> [spec:hfst:sem:fst.sfst.transducer.freely-insert-at-node-fn]
> Recursively adds a self-loop arc labelled `l` to every node reachable
> from `node`. If `node->was_visited(vmark)` is true, does nothing
> (already processed). Otherwise (now marked): adds a recursive arc
> `node->add_arc(l, node, this)` (a self-loop on this node labelled `l`),
> then iterates over all outgoing arcs of `node` (via ArcsIter) and
> recurses `freely_insert_at_node(arc->target_node(), l)` into each
> target. No return value. Caller must have freshly incremented `vmark`.
> Note: because the self-loop is added before iterating, the arc iterator
> will also traverse the newly added self-loop, but the `was_visited`
> guard prevents reprocessing this node.

> [spec:hfst:def:fst.sfst.transducer.generate-fn]
> void generate( FILE *file, int max=-1, OutputType ot=Joint )

> [spec:hfst:sem:fst.sfst.transducer.generate-fn]
> Enumerates and prints up to `max` accepting paths of this transducer to
> `file`, using a breadth-first worklist of `Gen` records. A `Gen` holds a
> `node`, a `previous` index (into the worklist, `undef` if none) and a
> `label`; `Gen::print` recursively prints its predecessor then emits this
> label according to the OutputType. Builds `vector<Gen> paths` seeded with
> `Gen(root_node())`. Sets a printed counter `n = 0`. Iterates `i` over
> `paths` by growing index (the vector grows during iteration): let
> `gen = paths[i]`, `node = gen.node`. If `node->is_final()`: if `ot ==
> Both`, call `gen.print(paths, file, alphabet, UpperOnly)`, write a tab,
> then `gen.print(..., LowerOnly)`; otherwise call `gen.print(paths, file,
> alphabet, ot)`. Write a newline. Increment `n`; if `n == max` return
> immediately. Then, regardless of finality, iterate over all arcs of
> `node` (epsilon first, via ArcsIter) and for each push
> `Gen(arc->target_node(), arc->label(), (Index)i)` onto `paths`. No return
> value. Note: cycles cause unbounded growth unless `max` cuts it off.
> `ot` defaults to `Joint`, `max` defaults to -1 (unlimited).

> [spec:hfst:def:fst.sfst.transducer.generate-string-fn]
> bool Transducer::generate_string( char *string, FILE *file, bool with_brackets)

> [spec:hfst:sem:fst.sfst.transducer.generate-string-fn]
> Generates surface forms from the analysis string `string` and prints them
> to `file`. Builds `Transducer a1(string, &alphabet, false)` (the linear
> transducer for `string`, tokenised with this transducer's alphabet, not
> extended). Composes `a2 = &(a1 || *this)` (a1 composed with this
> transducer). Takes the upper projection `a3 = &(a2->upper_level())`,
> deletes `a2`, then minimises `a2 = &a3->minimise()` and deletes `a3`.
> Copies this transducer's alphabet into `a2` (`a2->alphabet.copy(alphabet)`).
> Calls `result = a2->print_strings(file, with_brackets)`, deletes `a2`,
> and returns `result`. `with_brackets` defaults to true.

> [spec:hfst:def:fst.sfst.transducer.generates-empty-string-fn]
> bool generates_empty_string( void )

> [spec:hfst:sem:fst.sfst.transducer.generates-empty-string-fn]
> Returns true iff this transducer accepts the empty string. If not
> already `minimised`: builds `tmp = &minimise()`, reads `result =
> tmp->root_node()->is_final()`, deletes `tmp`, and returns `result`. If
> already minimised, returns `root_node()->is_final()` directly. (After
> minimisation the empty string is accepted iff the root node is final.)

> [spec:hfst:def:fst.sfst.transducer.incr-vmark-fn]
> void incr_vmark( void )

> [spec:hfst:sem:fst.sfst.transducer.incr-vmark-fn]
> Obtains a fresh visit marker for a new traversal by pre-incrementing the
> transducer's `vmark` field (`++vmark`). If the increment wraps around to
> 0 (overflow of the unsigned short), the old markers are stale: constructs
> an empty NodeHashSet `nodes`, calls `root.clear_visited(nodes)` to reset
> every reachable node's `visited` field to 0, prints `"clearing flags\n"`
> to stderr, and sets `vmark = 1`. No return value.

> [spec:hfst:def:fst.sfst.transducer.index-nodes-fn]
> void Transducer::index_nodes( Node *node, vector<Node*> *nodearray )

> [spec:hfst:sem:fst.sfst.transducer.index-nodes-fn]
> Recursive DFS that assigns each reachable node a sequential `index` and
> counts nodes and transitions. If `node->was_visited(vmark)` is true,
> returns immediately. Otherwise (now marked): sets `node->index =
> (Index)node_count` then post-increments `node_count`; if `nodearray` is
> non-NULL, pushes `node` onto it (so `(*nodearray)[index] == node`). Then
> iterates over all arcs of the node (epsilon first, via ArcsIter): for
> each arc increments `transition_count` and recurses
> `index_nodes(arc->target_node(), nodearray)`. No return value. Mutates
> the transducer's `node_count` and `transition_count` members; caller must
> have freshly incremented `vmark`.

> [spec:hfst:def:fst.sfst.transducer.infinitely-ambiguous-node-fn]
> bool infinitely_ambiguous_node( Node* )

> [spec:hfst:sem:fst.sfst.transducer.infinitely-ambiguous-node-fn]
> Recursively tests whether the subgraph reachable from `node` contains an
> epsilon-input cycle (which would make the transducer infinitely
> ambiguous). If `node->was_visited(vmark)` is true, returns false (already
> checked). Otherwise (now marked): constructs an empty NodeHashSet
> `visited` and calls `check_cyclicity(node, visited, alphabet)`; if that
> returns true, returns true. Then iterates over all arcs of `node`
> (epsilon first, via ArcsIter) and recurses
> `infinitely_ambiguous_node(arc->target_node())`; if any returns true,
> returns true. Otherwise returns false. (`check_cyclicity` is the helper
> detecting cycles whose labels are epsilon on the input side.)

> [spec:hfst:def:fst.sfst.transducer.is-automaton-fn]
> bool is_automaton( void )

> [spec:hfst:sem:fst.sfst.transducer.is-automaton-fn]
> Returns true iff this transducer is an automaton (every arc has identical
> upper and lower symbols). Calls `incr_vmark()` for a fresh marker, then
> returns `is_automaton_node(root_node())`.

> [spec:hfst:def:fst.sfst.transducer.is-automaton-node-fn]
> bool is_automaton_node( Node* )

> [spec:hfst:sem:fst.sfst.transducer.is-automaton-node-fn]
> Recursively tests whether every arc reachable from `node` has equal upper
> and lower characters. If `node->was_visited(vmark)` is true, returns true
> (already checked, no contradiction found here). Otherwise (now marked):
> iterates over all arcs of `node` (epsilon first, via ArcsIter); for each
> arc let `l = arc->label()`: if `l.upper_char() != l.lower_char()` return
> false; else recurse `is_automaton_node(arc->target_node())` and return
> false if it returns false. If the loop completes without contradiction,
> returns true. Caller must have freshly incremented `vmark`.

> [spec:hfst:def:fst.sfst.transducer.is-cyclic-fn]
> bool is_cyclic( void )

> [spec:hfst:sem:fst.sfst.transducer.is-cyclic-fn]
> Returns true iff this transducer's graph contains any directed cycle.
> Calls `incr_vmark()` for a fresh marker, constructs an empty NodeHashSet
> `previous`, and returns `is_cyclic_node(root_node(), previous)`.

> [spec:hfst:def:fst.sfst.transducer.is-cyclic-node-fn]
> bool is_cyclic_node( Node*, NodeHashSet &visited )

> [spec:hfst:sem:fst.sfst.transducer.is-cyclic-node-fn]
> Recursive DFS that detects a back-edge (cycle) reachable from `node`,
> using `previous` (a NodeHashSet) as the set of nodes on the current DFS
> path and `vmark`/`was_visited` to skip fully-explored subgraphs. If
> `node->was_visited(vmark)` is true, returns false. Otherwise (now
> marked): inserts `node` into `previous`, saving the returned iterator
> `it`. Iterates over all arcs of `node` (epsilon first, via ArcsIter): for
> each arc, if `arc->target_node()` is already in `previous` (find !=
> end()) OR `is_cyclic_node(arc->target_node(), previous)` returns true,
> return true. After the loop (no cycle found here) erases `it` from
> `previous` and returns false. (A local unused NodeHashSet `visited` is
> declared but not used.)

> [spec:hfst:def:fst.sfst.transducer.is-empty-fn]
> bool is_empty( void )

> [spec:hfst:sem:fst.sfst.transducer.is-empty-fn]
> Returns true iff this transducer accepts no string (empty language). If
> not already `minimised`: builds `tmp = &minimise()`, recurses
> `result = tmp->is_empty()`, deletes `tmp`, returns `result`. If already
> minimised: if `root_node()->is_final()` return false (accepts at least
> the empty string); otherwise return `root_node()->arcs()->is_empty()`
> (true iff the root has no outgoing arcs at all — for a minimised
> transducer this means the language is empty).

> [spec:hfst:def:fst.sfst.transducer.is-infinitely-ambiguous-fn]
> bool is_infinitely_ambiguous( void )

> [spec:hfst:sem:fst.sfst.transducer.is-infinitely-ambiguous-fn]
> Returns true iff this transducer is infinitely ambiguous (contains an
> epsilon-input cycle). Calls `incr_vmark()` for a fresh marker, then
> returns `infinitely_ambiguous_node(root_node())`.

> [spec:hfst:def:fst.sfst.transducer.label-set]
> typedef set<Label, Label::label_cmp> LabelSet

> [spec:hfst:def:fst.sfst.transducer.map-nodes-fn]
> void map_nodes( Node *node, Node *node2, Transducer *a, Level level )

> [spec:hfst:sem:fst.sfst.transducer.map-nodes-fn]
> Recursive projection copy that maps node `node` (this transducer) onto
> `node2` (a node already created in target transducer `a`), copying arcs
> whose labels are projected to a single level given by `level`. If
> `node->was_visited(vmark)` is true, returns immediately. Otherwise (now
> marked): records the mapping via `node->set_forward(node2)`; if
> `node->is_final()` marks `node2->set_final(1)`. Then iterates over all
> arcs of `node` (epsilon first, via ArcsIter): for each arc, build the
> projected label `Label l(arc->label().get_char(level))` (single character
> for the chosen level, used as both sides). Let `t = arc->target_node()`;
> if `t->check_visited(vmark)` (already visited, leaving its flag
> unchanged) reuse its mapped node `t2 = t->forward()`, else create
> `t2 = a->new_node()`. Add `node2->add_arc(l, t2, a)`, then recurse
> `map_nodes(t, t2, a, level)`. No return value.

> [spec:hfst:def:fst.sfst.transducer.minimise-alphabet-fn]
> void Transducer::minimise_alphabet()

> [spec:hfst:sem:fst.sfst.transducer.minimise-alphabet-fn]
> Rebuilds `alphabet` to contain exactly the symbols and labels actually
> used on the transducer's arcs. Declares an empty `SymbolMap symbols`
> (Character -> char*) and an empty `LabelSet labels`. Calls `incr_vmark()`,
> then `store_symbols(root_node(), symbols, labels)` which walks the graph
> collecting used symbol codes (mapped to freshly strdup'd names) and used
> labels. Calls `alphabet.clear()` to empty the alphabet. Then iterates
> `symbols`: for each entry calls `alphabet.add_symbol(it->second,
> it->first)` (re-registering name for code) and then `free(it->second)`
> (frees the strdup'd string). Finally iterates `labels` calling
> `alphabet.insert(*it)` for each. No return value.

> [spec:hfst:def:fst.sfst.transducer.negate-nodes-fn]
> void negate_nodes( Node*, Node* )

> [spec:hfst:sem:fst.sfst.transducer.negate-nodes-fn]
> In-place complementation helper over the (minimised, deterministic)
> subgraph rooted at `node`; `accept` is a universal accepting sink node.
> If `node->was_visited(vmark)` is true, returns immediately. Otherwise
> (now marked): flips the node's finality via
> `node->set_final(!node->is_final())`. Iterates over all arcs of `node`
> (epsilon first, via ArcsIter) and recurses
> `negate_nodes(arc->target_node(), accept)`. Then iterates over every
> label in this transducer's `alphabet`: for each label, if the node has no
> outgoing arc with that label (`!node->target_node(*it)`), adds an arc
> `node->add_arc(*it, accept, this)` to the accepting sink (so previously
> missing/rejecting transitions now lead to acceptance). No return value.

> [spec:hfst:def:fst.sfst.transducer.new-arc-fn]
> Arc *Transducer::new_arc( Label l, Node *target )

> [spec:hfst:sem:fst.sfst.transducer.new-arc-fn]
> Allocates a new Arc from the transducer's memory pool: `arc =
> (Arc*)mem.alloc(sizeof(Arc))`, calls `arc->init(l, target)` (sets the
> arc's label to `l` and target to `target`, leaving `next` untouched), and
> returns the pointer. The arc is owned by the pool, not separately freed.

> [spec:hfst:def:fst.sfst.transducer.new-node-fn]
> Node *Transducer::new_node()

> [spec:hfst:sem:fst.sfst.transducer.new-node-fn]
> Allocates a new Node from the transducer's memory pool: `node =
> (Node*)mem.alloc(sizeof(Node))`, calls `node->init()` (final=false,
> visited=0, empty arc lists, forwardp=NULL; `index` left uninitialised),
> and returns the pointer. The node is owned by the pool, not separately
> freed.

> [spec:hfst:def:fst.sfst.transducer.nodeindexing-fn]
> std::pair<size_t,size_t> Transducer::nodeindexing( vector<Node*> *nodearray )

> [spec:hfst:sem:fst.sfst.transducer.nodeindexing-fn]
> Ensures every reachable node has an assigned `index` and returns the
> node/transition counts. If the transducer is not already `indexed`: calls
> `incr_vmark()` for a fresh marker, calls `index_nodes(root_node(),
> nodearray)` (which assigns sequential indices, accumulates `node_count`
> and `transition_count`, and optionally fills `nodearray`), and sets
> `indexed = true`. Returns `std::pair<size_t,size_t>(node_count,
> transition_count)`. If already indexed, it skips the walk and just
> returns the existing counts (so `nodearray` is NOT populated in that
> case). `nodearray` defaults to NULL.

> [spec:hfst:def:fst.sfst.transducer.operator-fn]
> bool operator==( Transducer& )

> [spec:hfst:sem:fst.sfst.transducer.operator-fn]
> Equality test: returns true iff this transducer and `a` denote the same
> relation, minimising both operands first. Sets `p1 = this` if this is
> already `minimised`, else `p1 = &minimise()` (a fresh minimised copy);
> likewise `p2 = &a` if `a.minimised`, else `p2 = &a.minimise()`. Calls
> `p1->incr_vmark()` and `p2->incr_vmark()` (fresh markers for both), then
> `result = p1->compare_nodes(p1->root_node(), p2->root_node(), *p2)`
> (recursive structural equivalence of the two deterministic minimal
> graphs). If `p1 != this` deletes `p1`; if `p2 != &a` deletes `p2` (frees
> any temporary minimised copies). Returns `result`.

> [spec:hfst:def:fst.sfst.transducer.print-strings-fn]
> int Transducer::print_strings( FILE *file, bool with_brackets )

> [spec:hfst:sem:fst.sfst.transducer.print-strings-fn]
> Enumerates and prints every accepting path string of this transducer to
> `file`. Allocates a local `char buffer[BUFFER_SIZE]`, calls
> `incr_vmark()` for a fresh marker, and returns
> `print_strings_node(root_node(), buffer, 0, file, with_brackets)`.
> Returns the int from that call: nonzero (1) iff at least one accepting
> string was printed (it also warns on cycles). `with_brackets` defaults to
> true and controls whether labels are written with bracket markup.

> [spec:hfst:def:fst.sfst.transducer.print-strings-node-fn]
> int Transducer::print_strings_node(Node *node, char *buffer, int pos,

> [spec:hfst:sem:fst.sfst.transducer.print-strings-node-fn]
> Recursive DFS that prints all accepting path strings from `node`,
> building the current string into `buffer` (`pos` = current length) and
> writing complete strings to `file`. Initializes `result = 0`. Cycle
> handling: if `node->was_visited(vmark)` is true, then if
> `node->forward() != NULL` a cycle is detected — prints
> `"Warning: cyclic analyses (cycle aborted)\n"` to cerr and returns 0;
> otherwise sets `node->set_forward(node)` as an on-path flag. If
> `pos == BUFFER_SIZE` throws "Output string in function print_strings_node
> is too long". If `node->is_final()`: writes `buffer[pos]='\0'`,
> `fprintf(file, "%s\n", buffer)`, and sets `result = 1`. Then iterates
> over all arcs (epsilon first, via ArcsIter): for each arc, copy `p = pos`,
> let `l = arc->label()`, call `alphabet.write_label(l, buffer, &p,
> with_brackets)` (appends the label's text to buffer, advancing `p`), then
> OR into `result` the recursion `print_strings_node(arc->target_node(),
> buffer, p, file, with_brackets)`. After the loop clears the flag
> `node->set_forward(NULL)` and returns `result`. Note: the `was_visited`
> mark plus the `forward` flag together allow re-entry on different paths
> while still aborting genuine cycles.

> [spec:hfst:def:fst.sfst.transducer.read-fn]
> void read( FILE* )

> [spec:hfst:sem:fst.sfst.transducer.read-fn]
> Member function declared in fst.h (`void read(FILE*)`, "reads a
> transducer in binary format") but with no definition anywhere in the SFST
> back-end or libhfst sources. It is never defined and never called; no
> behaviour to port. (A placeholder/dead declaration; binary reading is
> done by `read_transducer_binary` and the `Transducer(FILE*, bool)`
> constructor instead.)

> [spec:hfst:def:fst.sfst.transducer.read-transducer-binary-fn]
> void Transducer::read_transducer_binary( FILE *file )

> [spec:hfst:sem:fst.sfst.transducer.read-transducer-binary-fn]
> Reads this transducer from binary file `file` (the standard SFST format).
> Reads the format tag: `fgetc(file)` must equal `'a'`, else throws
> "Error: wrong file format (not a standard transducer)\n". Sets `vmark = 0`
> and `deterministic = 0`. Reads `unsigned int n` (node count) via fread;
> if fread != 1 throws "read_transducer_binary: fread failed"; if
> `ferror(file)` throws "Error encountered while reading transducer from
> file". Allocates `Node **p = new Node*[n]` (index-to-node table), sets
> `p[0] = root_node()` and `p[i] = NULL` for `i` in 1..n-1. Calls
> `read_node(file, root_node(), p, this)` to read the root node and its
> whole subtree (allocating new nodes for non-NULL targets). Frees `p`
> (`delete[] p`). Reads the alphabet via `alphabet.read(file)`. Finally
> sets `vmark = 1` and `deterministic = minimised = 1`. No return value.

> [spec:hfst:def:fst.sfst.transducer.read-transducer-text-fn]
> void Transducer::read_transducer_text( FILE *file )

> [spec:hfst:sem:fst.sfst.transducer.read-transducer-text-fn]
> Reads this transducer from a text (AT&T-style) file `file`. Initializes
> `vector<Node*> nodes` with `root_node()` pushed (so node number 0 is the
> root). Sets `vmark = 0` and `deterministic = 0`. Reads the file line by
> line with `fgets(buffer, 10000, file)`, incrementing a `line` counter
> each iteration. For each line: set cursor `p = buffer`, parse the first
> field `s = next_string(p, line)` and obtain `node = create_node(nodes, s,
> line)` (the source node, created if new). If after that `p == NULL` (no
> further fields — a final-state line), mark `node->set_final(true)`.
> Otherwise: parse the target field and obtain `target = create_node(nodes,
> s, line)`; parse the lower-symbol field and `lc =
> alphabet.add_symbol(s)`; parse the upper-symbol field and `uc =
> alphabet.add_symbol(s)`; build `Label l(lc, uc)`; if `l == Label::epsilon`
> call `error_message(line)` (throws); else `alphabet.insert(l)` and
> `node->add_arc(l, target, this)`. After all lines, sets `vmark = 1` and
> `deterministic = minimised = 1`. No return value.

> [spec:hfst:def:fst.sfst.transducer.rec-cat-nodes-fn]
> void rec_cat_nodes( Node*, Node* )

> [spec:hfst:sem:fst.sfst.transducer.rec-cat-nodes-fn]
> Concatenation helper: links every final node reachable from `node` to
> `node2` via an epsilon arc, making them non-final. If
> `node->was_visited(vmark)` is true, returns immediately. Otherwise (now
> marked): first iterates over all arcs of `node` (epsilon first, via
> ArcsIter) recursing `rec_cat_nodes(arc->target_node(), node2)` (so the
> recursion happens before the relink). Then, if `node->is_final()`, clears
> its finality `node->set_final(0)` and adds an epsilon arc
> `node->add_arc(Label(), node2, this)` to `node2`. No return value. Caller
> must have freshly incremented `vmark`.

> [spec:hfst:def:fst.sfst.transducer.recode-label-fn]
> Label recode_label( Label, bool lswitch, bool recode, Alphabet& )

> [spec:hfst:sem:fst.sfst.transducer.recode-label-fn]
> Transforms a label `l` for copying into a target alphabet `al`. If
> `lswitch` is true, swaps the label's sides:
> `l = Label(l.upper_char(), l.lower_char())`. If `recode` is true, maps
> each side's character through this transducer's `alphabet` to its symbol
> name and re-registers it in `al`: `lc = al.add_symbol(alphabet.code2symbol(
> l.lower_char()))`, `uc = al.add_symbol(alphabet.code2symbol(
> l.upper_char()))`, set `l = Label(lc, uc)`, and `al.insert(l)`. (Switch
> is applied before recode, so recode reads the already-switched sides.)
> Returns the resulting (possibly switched and/or recoded) Label.

> [spec:hfst:def:fst.sfst.transducer.replace-char2-fn]
> void replace_char2( Node*, Node*, Character, Character, Transducer* )

> [spec:hfst:sem:fst.sfst.transducer.replace-char2-fn]
> Recursive copy that rebuilds the subgraph rooted at `node` (this
> transducer) into `node2` (a node in target transducer `a`), replacing
> character `c` by `nc` on every arc label. If `node->was_visited(vmark)`
> is true, returns immediately. Otherwise (now marked): records the mapping
> via `node->set_forward(node2)`; if `node->is_final()` sets
> `node2->set_final(1)`. Iterates over all arcs of `node` (epsilon first,
> via ArcsIter): for each, let `t = arc->target_node()`; if
> `t->check_visited(vmark)` (already visited, leaving flag unchanged) reuse
> `t2 = t->forward()`, else create `t2 = a->new_node()`. Add
> `node2->add_arc(arc->label().replace_char(c, nc), t2, a)` (the label with
> every occurrence of `c` replaced by `nc`), then recurse
> `replace_char2(t, t2, c, nc, a)`. No return value.

> [spec:hfst:def:fst.sfst.transducer.reverse-node-fn]
> void reverse_node( Node *old_node, Transducer *new_node )

> [spec:hfst:sem:fst.sfst.transducer.reverse-node-fn]
> Recursive helper that builds the reversed automaton of the subgraph
> rooted at `node` into transducer `na`. Uses each original node's
> `forward` pointer to hold its counterpart node in `na`. If
> `node->was_visited(vmark)` is true, returns immediately. Otherwise (now
> marked): creates the counterpart `node->set_forward(na->new_node())`. If
> `node->is_final()`, adds an epsilon arc from `na`'s root to this
> counterpart: `na->root_node()->add_arc(Label(), node->forward(), na)` (so
> original final states become start states of the reverse). Then iterates
> over all arcs of `node` (epsilon first, via ArcsIter): for each arc,
> first recurse `reverse_node(arc->target_node(), na)` (ensuring the
> target's counterpart exists), let `n = arc->target_node()->forward()`,
> and create the reversed arc `n->add_arc(arc->label(), node->forward(),
> na)` (from target's counterpart back to this node's counterpart). No
> return value. (The caller marks the original root's counterpart final
> after this returns.)

> [spec:hfst:def:fst.sfst.transducer.root-node-fn]
> Node *root_node( void )

> [spec:hfst:sem:fst.sfst.transducer.root-node-fn]
> Accessor: returns a pointer to the transducer's embedded `root` node,
> `&root`. Two overloads, non-const returning `Node*` and const returning
> `const Node*`; both return the address of the `root` member.

> [spec:hfst:def:fst.sfst.transducer.size-fn]
> size_t Transducer::size()

> [spec:hfst:sem:fst.sfst.transducer.size-fn]
> Returns the number of distinct nodes in the transducer. Calls
> `incr_vmark()` for a fresh marker, then returns
> `size_node(root_node())` (the recursive count of reachable nodes).

> [spec:hfst:def:fst.sfst.transducer.size-node-fn]
> size_t Transducer::size_node( Node *node )

> [spec:hfst:sem:fst.sfst.transducer.size-node-fn]
> Recursively counts the number of distinct nodes reachable from `node`,
> using `vmark`/`was_visited` to count each node at most once. Initializes
> `result = 0`. If `node->was_visited(vmark)` is true (already counted),
> falls through and returns 0 for this node. Otherwise (now marked):
> increments `result` to 1, then iterates over all arcs of `node` (epsilon
> first, via ArcsIter) and for each adds `size_node(arc->target_node())`
> into `result`. Returns `result` (this node plus the subtotals of all
> reachable-but-not-yet-counted descendants). Caller must have freshly
> incremented `vmark`.

> [spec:hfst:def:fst.sfst.transducer.splice-arc-fn]
> void splice_arc( Node*, Node*, Node*, Transducer* )

> [spec:hfst:sem:fst.sfst.transducer.splice-arc-fn]
> Recursive helper used by splice insertion (defined in operators.cc).
> Copies the subgraph rooted at `node` (a node of the spliced-in
> transducer) into target transducer `a`, attaching it under `node2`, and
> redirecting each of the spliced transducer's final states to
> `next_node`. If `node->is_final()`: adds an epsilon arc
> `node2->add_arc(Label(), next_node, a)` (linking this final node onward
> to `next_node`) and returns. Otherwise iterates over all arcs of `node`
> (epsilon first, via ArcsIter): for each arc, creates a fresh node
> `tn = a->new_node()`, adds `node2->add_arc(arc->label(), tn, a)`, then
> recurses `splice_arc(arc->target_node(), tn, next_node, a)`. No return
> value. (Note: no visited-marking, so the spliced subgraph is assumed
> acyclic; it is fully unrolled/copied.)

> [spec:hfst:def:fst.sfst.transducer.splice-nodes-fn]
> void splice_nodes(Node*, Node*, Label sl, Transducer*, Transducer*)

> [spec:hfst:sem:fst.sfst.transducer.splice-nodes-fn]
> Recursive copy that rebuilds the subgraph rooted at `node` (this
> transducer) into `node2` (a node of target transducer `a`), replacing
> every arc whose label equals the splice label `sl` with an inserted copy
> of transducer `sa` (defined in operators.cc). Uses each original node's
> `forward` pointer to hold its counterpart in `a`. If
> `node->was_visited(vmark)` is true, returns immediately. Otherwise (now
> marked): records `node->set_forward(node2)`; if `node->is_final()` sets
> `node2->set_final(1)`. Iterates over all arcs of `node` (epsilon first,
> via ArcsIter): for each arc let `t = arc->target_node()`; if
> `t->check_visited(vmark)` (already visited, flag unchanged) reuse
> `t2 = t->forward()`, else create `t2 = a->new_node()`. If
> `arc->label() == sl`, splice in `sa` by calling
> `splice_arc(sa->root_node(), node2, t2, a)` (which copies `sa`'s graph
> between `node2` and `t2`); otherwise add a plain link
> `node2->add_arc(arc->label(), t2, a)`. Then recurse
> `splice_nodes(t, t2, sl, sa, a)`. No return value. Caller must have
> freshly incremented `vmark`.

> [spec:hfst:def:fst.sfst.transducer.store-fn]
> void Transducer::store( FILE *file )

> [spec:hfst:sem:fst.sfst.transducer.store-fn]
> Writes this transducer to binary file `file` in the standard SFST
> format. Writes the format tag byte `fputc('a', file)`. Declares an empty
> `vector<Node*> nodearray` and calls
> `indexing_pair = nodeindexing(&nodearray)` to assign node indices and get
> `(node_count, transition_count)` (an HFST modification uses the returned
> pair's `.first` for the node count rather than `nodearray.size()`, since
> nodeindexing may not always populate the array). Calls `incr_vmark()` for
> a fresh marker. Fwrites the node count `n = (unsigned int)
> indexing_pair.first` as an `unsigned int`. Calls
> `store_node(file, root_node(), vmark)` to recursively write the whole
> graph (final flags, arc counts, arc labels and target node indices).
> Finally writes the alphabet via `alphabet.store(file)`. No return value.

> [spec:hfst:def:fst.sfst.transducer.store-lowmem-fn]
> void Transducer::store_lowmem( FILE *file )

> [spec:hfst:sem:fst.sfst.transducer.store-lowmem-fn]
> Writes this transducer to binary file `file` in the low-memory format
> (nodes addressed by byte offset rather than index, enabling on-demand
> loading). Writes the format tag byte `fputc('l', file)`, then writes the
> alphabet via `alphabet.store(file)`. Declares `vector<Node*> nodearray`
> and calls `nodeindexing(&nodearray)` to assign indices and fill the array
> in index order. Computes the file start offset of each node: reads the
> current position `pos = (unsigned int)ftell(file)`, and for each node `i`
> in `nodearray` pushes `pos` onto `startpos` then advances `pos` by that
> node's serialized size = `sizeof(char)` (final flag) +
> `sizeof(unsigned short)` (arc count) + `arcs->size() * (sizeof(Character)
> * 2 + sizeof(unsigned int))` (each arc: lower Character, upper Character,
> target offset). After computing `startpos`, iterates `i` over
> `nodearray` again calling `store_lowmem_node(file, nodearray[i],
> startpos)` to write each node (final flag, arc count, and each arc's
> labels followed by the target node's precomputed byte offset). No return
> value.

> [spec:hfst:def:fst.sfst.transducer.store-symbols-fn]
> void Transducer::store_symbols(Node *node, SymbolMap &symbol,

> [spec:hfst:sem:fst.sfst.transducer.store-symbols-fn]
> Recursively collects the symbol codes and labels actually used on the
> arcs reachable from `node` into `symbol` (a SymbolMap, Character ->
> char*) and `labels` (a LabelSet). If `node->was_visited(vmark)` is true,
> does nothing. Otherwise (now marked, via the `was_visited` test):
> iterates over all arcs of `node` (epsilon first, via ArcsIter); for each
> arc let `l = arc->label()`: inserts `l` into `labels`; then for the upper
> character `c = l.upper_char()`, if `c` is not already a key in `symbol`,
> looks up its name `s = alphabet.code2symbol(c)` and, if non-NULL, stores
> `symbol[c] = fst_strdup(s)` (a freshly duplicated copy of the name);
> repeats the same for the lower character `c = l.lower_char()`. Then
> recurses `store_symbols(arc->target_node(), symbol, labels)`. No return
> value. Caller must have freshly incremented `vmark`; the strdup'd strings
> are owned by `symbol` and freed later by the caller.

> [spec:hfst:def:fst.sfst.transducer.symbol-map]
> typedef hash_map<Character, char*> SymbolMap

> [spec:hfst:def:fst.sfst.transducer.transducer-fn]
> Transducer::Transducer( istream &is, const Alphabet *a, bool verbose,

> [spec:hfst:sem:fst.sfst.transducer.transducer-fn]
> Constructor that builds a transducer from an input stream `is` of words,
> one per line, by unioning each word's accepting path (member-initializes
> `root()` and `mem()`). Locals: `bool extended = false`, `int n = 0`,
> `char buffer[10000]`. Initializes members: `vmark = 0`, `indexed =
> false`, `node_count = transition_count = 0`, `deterministic = true`,
> `minimised = false`. If alphabet `a` is non-NULL, copies it
> (`alphabet.copy(*a)`) and sets `extended = true`. Then loops reading
> lines with `is.getline(buffer, 10000)`: if `verbose` and `++n` is a
> multiple of 10000, prints progress to cerr (a newline at n==10000, then
> `"\r" << n << " words"`). If `lexcomments` is true, strips comments:
> scans `buffer`; a backslash followed by a non-NUL char is a quoted
> character (skipped), and a `'%'` begins a comment — truncates the buffer
> there (`buffer[i]=0`) and breaks; if the line is now empty (`buffer[0]==
> 0`) continues to the next line. Then trims trailing whitespace (space,
> tab, '\r') from the end, but stops trimming if the preceding char is a
> backslash (quoted whitespace), writing a NUL after the last kept char.
> Calls `add_string(buffer, extended)` to add that word's path. After the
> loop, if `verbose` and `n >= 10000` prints a trailing newline to cerr.
> Parameters `verbose` and `lexcomments` control progress output and
> comment handling respectively.

> [spec:hfst:def:fst.sfst.v-type]
> typedef unsigned short VType

