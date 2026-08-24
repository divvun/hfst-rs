# General-lookup termination

The general lookup engine — the one that runs on any machine, as opposed to the
packed tables of `docs/spec/ol-lookup-enumeration.md` — is a depth-first walk of
a transition graph.  Two kinds of arc consume no input: an input epsilon, and a
flag diacritic, which spends its turn rewriting the walk's flag registers
instead.  Those are the arcs that can return the walk to where it already
stands, and they are the whole of the termination problem: an arc that consumes
input can be followed at most as many times as the input is long.

The measured case is a Giella replace rule, `downcase-derived_proper-strings`:
eight states, four flag diacritics, cyclic away from the initial state, and a
downcasing relation underneath.  Looking up the single character `A` never
returned.  The engine's only bound on non-consuming arcs was a count of cycle
re-entries — five by default — and a count does not bound a tree.  Every one of
the four flags is satisfiable on every turn (two set a feature outright, two
unify a feature with the value they themselves set), so each situation the walk
stands in offers four more non-consuming arcs, and the count merely fixes how
deep that tree may grow.  Measured at successive caps, the walk enumerated 191,
3,364, 55,703 and 902,360 readings — a factor of about sixteen per permitted
turn, putting the default cap of five at a quarter of a billion.  Every one of
them was one of the same two analyses: `A` and `a`, both at weight zero.
Upstream C++ HFST 3.17.1 does not return on this input either.

The rules below constrain the walk's termination.  They do not license it to
change the relation it walks.

## A cycle that consumed nothing is refused

> [spec:hfst:req:general-lookup-termination.non-progressing-cycle]
> An arc that consumes no input symbol — an epsilon arc, or a flag diacritic —
> and that lands the walk in a situation already on the current depth-first path
> MUST NOT be followed.  A situation is the pair of the state the walk stands in
> and the flag configuration in force there: the feature values held and their
> polarities.  Those two settle what the walk may do next, so a second arrival
> at one of them begins a sub-search identical to the one already running above
> it, and following it can only repeat work already in hand.  The trap MUST key
> on the flag configuration and not on the state alone — a flag arc that
> rewrites a register has moved the walk to a genuinely different situation even
> when the state repeats, and cutting it would lose the readings that need two
> flag arcs through one state.  It MUST key on the evaluated configuration and
> not on the sequence of flag symbols traversed, which grows on every turn of a
> loop and so never repeats.

> [spec:hfst:req:general-lookup-termination.progress-resets-the-trap]
> Consuming an input symbol is progress, and MUST empty the trap for the walk
> below that arc: every situation recorded before it is legitimately reachable
> again afterwards, at a position further along the input.  A machine whose only
> arc is a self-loop consuming one symbol must be walked once per input symbol.
> The trap MUST be restored when that descent returns, so that it always holds
> exactly the situations on the current path of non-consuming arcs, and nothing
> else.

## Termination is not a budget

> [spec:hfst:req:general-lookup-termination.no-cycle-count-termination]
> The walk MUST terminate on every machine whatever cycle count, result count or
> time cutoff the caller supplied, and MUST NOT rest its termination on any of
> them.  A caller's cycle cap bounds how much of an infinite family of readings
> is enumerated; it is not a safety net, and a walk that only terminates because
> a counter ran out will not terminate when the counter is raised — the counter
> is exactly the exponent of the work it permits.  Termination MUST instead
> follow from the finiteness of the situations the trap above admits: states are
> finite, and so are the flag configurations a machine's own diacritics can
> reach, so the path of non-consuming arcs is bounded by their product.

## What the trap costs

> [spec:hfst:sem:general-lookup-termination.enumeration-divergence]
> An input-epsilon cycle that writes output offers infinitely many readings of
> one input, and the trap ends that family after one turn: `A [0:x]*` answers
> `A` and `Ax`, where the upstream engine answers one reading per permitted
> cycle and then stops at the cap.  No analysis reachable without repeating a
> situation is lost, and no minimum weight is lost in a semiring where going
> round again cannot help — in the tropical semiring a cycle's weight is
> non-negative, so the first turn is always the cheapest, and a machine with a
> negative cycle has no minimum to report.  The conformance target is the
> optimized-lookup engine, which bounds its walk the same way: the same relation
> held as a transition graph and as a packed table now answers identically,
> where before the two disagreed on how many turns of a loop to report.  A
> caller who wants the family enumerated should ask the machine for its paths,
> not its analyses of one input.
