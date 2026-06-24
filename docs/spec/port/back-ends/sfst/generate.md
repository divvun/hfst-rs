# back-ends/sfst/generate.cc

> [spec:hfst:def:generate.sfst.gen]
> class Gen {
>   Node *node;
>   Index previous;
>   Label label;
> }

> [spec:hfst:def:generate.sfst.gen.gen-fn]
> Gen( Node *n, Label l=Label::epsilon , Index p=undef )

> [spec:hfst:sem:generate.sfst.gen.gen-fn]
> Constructor for `Gen`. Takes a `Node *n` (required), a `Label l`
> defaulting to `Label::epsilon`, and an `Index p` defaulting to `undef`.
> Initializes the members directly: `node = n`, `previous = p`,
> `label = l`. Note the member initializer list order differs from the
> parameter order (node, previous, label), but members are initialized in
> declaration order (node, previous, label) regardless. No other work, no
> side effects.

> [spec:hfst:def:generate.sfst.gen.print-fn]
> void print( vector<Gen> &paths, FILE *file, Alphabet &alphabet,

> [spec:hfst:sem:generate.sfst.gen.print-fn]
> Recursively prints the labels along the path that leads to this `Gen`
> node, in order from the root to this node, by writing to `file`.
> Parameters: `paths` is the vector of all `Gen` entries (used to follow
> the `previous` back-link), `file` is the output stream, `alphabet`
> provides label/character-to-string conversion, and `ot` is the
> `OutputType` selecting which side to emit.
> Step by step: If `this->previous == undef`, do nothing (this is the
> root; base case). Otherwise, first recursively call
> `paths[previous].print(paths, file, alphabet, ot)` to print the prefix
> of the path. Then emit this node's incoming label `label` according to
> `ot`:
> - `Joint`: write `alphabet.write_label(label)` (the full upper:lower
>   label string) via `fputs`.
> - `UpperOnly`: if `label.upper_char() != Label::epsilon`, write
>   `alphabet.write_char(label.upper_char())` via `fputs`; skip epsilon.
> - `LowerOnly`: if `label.lower_char() != Label::epsilon`, write
>   `alphabet.write_char(label.lower_char())` via `fputs`; skip epsilon.
> No other `ot` values are handled here. Mutates nothing; only writes to
> `file`. Note: recursion depth equals the path length, so very long paths
> recurse deeply.

> [spec:hfst:def:generate.sfst.transducer.generate-fn]
> void Transducer::generate( FILE *file, int max, OutputType ot )

> [spec:hfst:sem:generate.sfst.transducer.generate-fn]
> Enumerates accepted paths of the transducer by breadth-first expansion
> and prints each accepted (final-state) path to `file`. Parameters:
> `file` output stream, `max` maximum number of paths to print (0 or
> negative means effectively unlimited since the counter never matches a
> non-reached value — but the loop still terminates when paths run out),
> `ot` the `OutputType`.
> Step by step:
> 1. Create a local `vector<Gen> paths` and push one initial `Gen` for the
>    transducer's `root_node()` (with default epsilon label and `undef`
>    previous).
> 2. Initialize printed-count `n = 0`.
> 3. Iterate over `paths` by index `i` from 0 upward, where the vector
>    grows during iteration (a worklist; `paths.size()` is re-evaluated
>    each iteration). Let `gen = paths[i]` and `node = gen.node`.
> 4. If `node->is_final()`:
>    - If `ot == Both`: call `gen.print(paths, file, alphabet, UpperOnly)`,
>      then write a tab `'\t'`, then call
>      `gen.print(paths, file, alphabet, LowerOnly)`.
>    - Otherwise: call `gen.print(paths, file, alphabet, ot)`.
>    Then write a newline `'\n'`. Increment `n`; if `++n == max`, return
>    immediately (stops generation after printing `max` paths).
> 5. Regardless of finality, iterate over the node's outgoing arcs via
>    `ArcsIter`; for each arc push a new `Gen(arc->target_node(),
>    arc->label(), (Index)i)` onto `paths`, linking back to the current
>    index `i` as `previous`.
> 6. The loop ends when `i` reaches the end of `paths` (no more frontier).
> Note: this performs no cycle detection; a cyclic transducer would grow
> `paths` unboundedly unless bounded by `max`. The take care: `gen` is a
> reference into `paths`, but `print` is called before further
> `push_back` invalidation matters for that iteration; the captured
> `node` pointer and index `i` are used after pushes.

