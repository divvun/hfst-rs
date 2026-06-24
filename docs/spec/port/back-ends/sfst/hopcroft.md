# back-ends/sfst/hopcroft.cc

> [spec:hfst:def:hopcroft.sfst.minimiser]
> class Minimiser {
>   class Transition { public: Index source; Index next_for_target; Index next_for_label; Label label; // [spec:hfst:def:hopcroft.sfst.minimiser.transition.trans...;
>   class State { public: Index group; // index of group to which this state belongs Index next_in_group; // index of next state in group Index previous_in_group...;
>   class StateGroup { public: Index next; // index of next source group Index next_in_agenda; Index previous_in_agenda; Index size; // number of states in this ...;
>   class Agenda { static const Index bucket_count = (Index)(sizeof(Index) * 8); // the first "bucket_count" many groups are dummy groups // used as the agenda b...;
>   Transducer &transducer;
>   size_t number_of_nodes;
>   size_t number_of_transitions;
>   vector<Node*> nodearray;
>   vector<StateGroup> group;
>   vector<State> state;
>   vector<Transition> transition;
>   Agenda agenda;
>   Label2TransSet first_transition_for_label;
>   Index first_source_group;
>   Transducer &result();
>   Transducer &build_transducer();
> }

> [spec:hfst:def:hopcroft.sfst.minimiser.add-state-fn]
> void Minimiser::add_state( Index g, Index s )

> [spec:hfst:sem:hopcroft.sfst.minimiser.add-state-fn]
> Adds state index `s` to group index `g`.
> Steps: increment `group[g].size` by 1; set `state[s].group = g`;
> then call `link_state_in(group[g].first_state, s)` to link `s` into
> the circular doubly-linked state list of group `g` (passing
> `group[g].first_state` by reference so it is updated if the list was
> empty). No return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.add-transition-fn]
> void Minimiser::add_transition( Index s, Label l, Index t )

> [spec:hfst:sem:hopcroft.sfst.minimiser.add-transition-fn]
> Records an incoming transition with source state `s`, label `l`, and
> target state `t`, prepending it onto target `t`'s singly-linked list of
> incoming transitions.
> Steps: construct a `Transition T(s, l, state[t].first_transition)` (so
> `T.next_for_target` becomes the previous head of `t`'s incoming list and
> `T.next_for_label = undef`); set `state[t].first_transition` to the index
> the new transition will occupy, i.e. the current `transition.size()`;
> then `push_back(T)` onto the `transition` vector. No return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.agenda]
> class Agenda {
>   static const Index bucket_count = (Index)(sizeof(Index) * 8);
>   vector<StateGroup> &group;
> }

> [spec:hfst:def:hopcroft.sfst.minimiser.agenda.add-fn]
> void add( Index g, Index size )

> [spec:hfst:sem:hopcroft.sfst.minimiser.agenda.add-fn]
> Inserts group index `g` into the agenda, choosing a bucket by the
> magnitude of `size`.
> Steps: compute bucket index `i` = floor(log2(size)) by repeatedly
> right-shifting `size` by 1 and incrementing `i` until `size` becomes 0
> (loop `for(i=0; (size >>= 1); i++)`; note `size==0` yields `i==0` and
> `size==1` yields `i==0`). The bucket is the dummy group at index `i`.
> Then insert `g` at the head of bucket `i`'s circular agenda list:
> let `next = group[i].next_in_agenda`; set `group[i].next_in_agenda = g`;
> `group[g].next_in_agenda = next`; `group[g].previous_in_agenda = i`;
> `group[next].previous_in_agenda = g`. No return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.agenda.agenda-fn]
> Agenda( vector<StateGroup> &g ) : group(g)

> [spec:hfst:sem:hopcroft.sfst.minimiser.agenda.agenda-fn]
> Constructor. Binds the member reference `group` to the passed-in vector
> `g` (the Minimiser's shared `group` vector).
> Then allocates the dummy bucket groups: resizes `g` to `bucket_count`
> (= sizeof(Index)*8, i.e. one bucket per bit of an Index) so indices
> `0..bucket_count-1` are reserved as agenda buckets; for each
> `i` in `0..bucket_count-1`, sets `group[i].next_in_agenda` and
> `group[i].previous_in_agenda` both to `i` (each bucket is an empty
> circular list pointing to itself).

> [spec:hfst:def:hopcroft.sfst.minimiser.agenda.contains-fn]
> bool contains( Index g )

> [spec:hfst:sem:hopcroft.sfst.minimiser.agenda.contains-fn]
> Returns whether group index `g` is currently on the agenda: returns
> `group[g].next_in_agenda != g`. (A group not on the agenda has its
> agenda links pointing to itself, so a self-loop means absent.)

> [spec:hfst:def:hopcroft.sfst.minimiser.agenda.erase-fn]
> void erase( Index g )

> [spec:hfst:sem:hopcroft.sfst.minimiser.agenda.erase-fn]
> Unlinks group index `g` from whatever agenda bucket list it is in.
> Steps: let `next = group[g].next_in_agenda` and
> `previous = group[g].previous_in_agenda`; splice `g` out by setting
> `group[previous].next_in_agenda = next` and
> `group[next].previous_in_agenda = previous`; then reset `g`'s own links
> to point to itself: `group[g].previous_in_agenda = group[g].next_in_agenda = g`
> (marking it as not on the agenda). No return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.agenda.number-of-buckets-fn]
> Index number_of_buckets()

> [spec:hfst:sem:hopcroft.sfst.minimiser.agenda.number-of-buckets-fn]
> Returns the constant `bucket_count` (= sizeof(Index)*8), the number of
> dummy bucket groups reserved at the front of the group vector.

> [spec:hfst:def:hopcroft.sfst.minimiser.agenda.pop-fn]
> Index pop()

> [spec:hfst:sem:hopcroft.sfst.minimiser.agenda.pop-fn]
> Removes and returns one group from the agenda, scanning buckets from
> smallest bucket index upward.
> Steps: for `i` from 0 to `bucket_count-1`, if bucket `i` is non-empty
> (`group[i].next_in_agenda != i`), take `result = group[i].next_in_agenda`
> (the first group in that bucket), call `erase(result)` to unlink it, and
> return `result`. If every bucket is empty, return `undef`.

> [spec:hfst:def:hopcroft.sfst.minimiser.compute-source-states-fn]
> void Minimiser::compute_source_states( Index g )

> [spec:hfst:sem:hopcroft.sfst.minimiser.compute-source-states-fn]
> Builds, for the splitter group `g` (set C), the buckets of incoming
> transitions grouped by label, stored in `first_transition_for_label`.
> Steps: clear `first_transition_for_label`. Let `first =
> group[g].first_state` and iterate `s` over the circular state list of
> group `g` starting at `first` (do/while until `s` returns to `first`;
> assumes the group is non-empty). For each state `S = state[s]`, walk its
> singly-linked incoming-transition list via `t = S.first_transition`,
> following `transition[t].next_for_target` until `undef`. For each
> transition `T = transition[t]`: set `T.next_for_label = undef`, then look
> up `T.label` in `first_transition_for_label`; if absent, insert mapping
> `T.label -> t`; if present, prepend `t` to that label's list by setting
> `T.next_for_label = it->second` (the previous head) and updating the map
> entry to `t`. Advance `s = S.next_in_group`. No return value; mutates
> `first_transition_for_label` and the `next_for_label` fields.

> [spec:hfst:def:hopcroft.sfst.minimiser.first-group-fn]
> Index first_group()

> [spec:hfst:sem:hopcroft.sfst.minimiser.first-group-fn]
> Returns `agenda.number_of_buckets()` (= `bucket_count`), i.e. the index
> of the first real (non-dummy) group in the `group` vector. Real groups
> occupy indices `first_group() .. group.size()-1`.

> [spec:hfst:def:hopcroft.sfst.minimiser.label2-trans-set]
> typedef map<Label,Index> Label2TransSet

> [spec:hfst:def:hopcroft.sfst.minimiser.link-state-in-fn]
> void Minimiser::link_state_in( Index &first_state, Index s )

> [spec:hfst:sem:hopcroft.sfst.minimiser.link-state-in-fn]
> Links state index `s` into a circular doubly-linked state list whose head
> index is held in the by-reference parameter `first_state`.
> If the list is empty (`first_state == undef`): set `first_state = s` and
> make `s` a self-loop (`state[s].next_in_group = state[s].previous_in_group = s`).
> Otherwise insert `s` immediately after the head: let
> `n = state[first_state].next_in_group`; set
> `state[first_state].next_in_group = s`; `state[s].next_in_group = n`;
> `state[n].previous_in_group = s`; `state[s].previous_in_group = first_state`.
> (The head `first_state` itself is not changed when the list was
> non-empty.) No return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.link-state-out-fn]
> void Minimiser::link_state_out( Index &first_state, Index s )

> [spec:hfst:sem:hopcroft.sfst.minimiser.link-state-out-fn]
> Unlinks state index `s` from the circular doubly-linked state list whose
> head index is held in the by-reference parameter `first_state`.
> Let `S = state[s]`. If `s` is the only state in the list
> (`S.next_in_group == s`): set `first_state = undef`. Otherwise: let
> `p = S.previous_in_group` and `n = S.next_in_group`; splice `s` out by
> setting `state[p].next_in_group = n` and `state[n].previous_in_group = p`;
> if `s` was the head (`first_state == s`), advance the head to `n`
> (`first_state = n`). Does not reset `s`'s own next/previous links.
> No return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.merge-state-lists-fn]
> void Minimiser::merge_state_lists( Index g )

> [spec:hfst:sem:hopcroft.sfst.minimiser.merge-state-lists-fn]
> Merges group `g`'s "new" state list (`first_new_state`) back into its
> main state list (`first_state`), undoing a tentative split.
> Steps: let `first1 = group[g].first_state`. If `first1 == undef` (main
> list empty), just set `group[g].first_state = group[g].first_new_state`.
> Otherwise splice the two circular lists together: let
> `first2 = group[g].first_new_state`,
> `next1 = state[first1].next_in_group`, `next2 = state[first2].next_in_group`;
> set `state[first1].next_in_group = next2`,
> `state[first2].next_in_group = next1`,
> `state[next1].previous_in_group = first2`,
> `state[next2].previous_in_group = first1`.
> Then clear the new list: set `group[g].first_new_state = undef`, add
> `group[g].new_size` into `group[g].size`, and reset `group[g].new_size = 0`.
> Note: it does NOT update the `state[].group` field of the moved states
> (they already belong to `g`). No return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.minimiser-fn]
> Minimiser::Minimiser( Transducer &t )

> [spec:hfst:sem:hopcroft.sfst.minimiser.minimiser-fn]
> Constructor. Member initialisers: `transducer(t)` and `agenda(group)`
> (the latter resizes `group` to `bucket_count` dummy bucket groups; see
> the Agenda constructor).
> Body steps: call `t.nodeindexing(&nodearray)`, which fills `nodearray`
> (index -> Node*) and returns a `pair<size_t,size_t>` of (node count,
> transition count); store these into `number_of_nodes` and
> `number_of_transitions`. Resize `state` to `number_of_nodes`; reserve
> `number_of_transitions` slots in `transition`; reserve
> `number_of_nodes + first_group()` slots in `group`.
> Create the two initial real groups: push a `StateGroup`, set
> `final = ` its index, and call `init(final)` on it; push another
> `StateGroup`, set `nonfinal = ` its index, and call `init(nonfinal)`.
> Then iterate `sourceID` from 0 to `nodearray.size()-1`: let
> `node = nodearray[sourceID]`; if `node->is_final()` call
> `add_state(final, sourceID)` else `add_state(nonfinal, sourceID)`; then
> for each arc `p` of `node->arcs()`, call
> `add_transition(sourceID, arc->label(), arc->target_node()->index)` to
> record the (reverse-indexed) incoming transition list.

> [spec:hfst:def:hopcroft.sfst.minimiser.move-state-to-new-fn]
> void Minimiser::move_state_to_new( Index g, Index s )

> [spec:hfst:sem:hopcroft.sfst.minimiser.move-state-to-new-fn]
> Moves state index `s` out of group `g`'s main state list and into group
> `g`'s "new" (intersection) state list.
> Steps: decrement `group[g].size`; increment `group[g].new_size`; call
> `link_state_out(group[g].first_state, s)` to remove `s` from the main
> list; call `link_state_in(group[g].first_new_state, s)` to add `s` to the
> new list. Does not change `state[s].group`. No return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.print-groups-fn]
> void print_groups()

> [spec:hfst:sem:hopcroft.sfst.minimiser.print-groups-fn]
> Debug-only (compiled only when `NDEBUG` is not defined). Prints the
> contents of all real groups to stderr.
> Steps: print a separator line "--------------". For each group index `g`
> from `first_group()` to `group.size()-1`: print "group %lu: " using
> `g - first_group()` as the displayed number. If
> `group[g].first_state != undef`, walk the circular state list starting at
> it (do/while until returning to the head), printing each state index `s`
> followed by a space. If `group[g].first_new_state != undef`, print "| "
> then similarly walk and print the circular new-state list. End each group
> with a newline. No return value; output goes to stderr only.

> [spec:hfst:def:hopcroft.sfst.minimiser.process-source-groups-fn]
> void Minimiser::process_source_groups( Label l )

> [spec:hfst:sem:hopcroft.sfst.minimiser.process-source-groups-fn]
> Given a label `l`, processes all source states that have an incoming
> transition labelled `l` into the current splitter group, tentatively
> moving them into their groups' "new" lists, then deciding per group
> whether to split.
> Steps: set `first_source_group = undef`. Walk the per-label incoming list
> for `l`: starting at `t = first_transition_for_label[l]`, following
> `transition[t].next_for_label` until `undef`. For each transition
> `T = transition[t]`: let `S = state[T.source]` and `g = S.group`. If this
> group has no new states yet (`group[g].first_new_state == undef`), prepend
> `g` to the singly-linked source-group list via `group[g].next` field:
> `group[g].next = first_source_group; first_source_group = S.group`. Then
> call `move_state_to_new(g, T.source)`.
> After processing all transitions, walk the source-group list from
> `first_source_group` following `group[g].next`: for each group `g`, if
> `group[g].size > 0` (some states remained in the main list) call
> `split(g, l)`; otherwise (all states moved) call `merge_state_lists(g)`
> to restore the group unchanged. Returns void.

> [spec:hfst:def:hopcroft.sfst.minimiser.remove-state-fn]
> void Minimiser::remove_state( Index g, Index s )

> [spec:hfst:sem:hopcroft.sfst.minimiser.remove-state-fn]
> Removes state index `s` from group index `g`.
> Steps: decrement `group[g].size`; call
> `link_state_out(group[g].first_state, s)` to unlink `s` from the group's
> circular state list (updating the head by reference if needed). No return
> value.

> [spec:hfst:def:hopcroft.sfst.minimiser.split-fn]
> void Minimiser::split( Index g, Label l )

> [spec:hfst:sem:hopcroft.sfst.minimiser.split-fn]
> Splits group `g` by promoting its "new" (intersection) state list into a
> brand-new group, then updates the agenda. The `l` parameter is unused in
> the body.
> Steps: let `newg = group.size()` and push a new `StateGroup` `NewG`; call
> `NewG.init(newg)`. Move `g`'s new list into `NewG`:
> `NewG.first_state = group[g].first_new_state`, `NewG.size = group[g].new_size`;
> then clear `group[g].first_new_state = undef` and `group[g].new_size = 0`.
> (CAVEAT: pushing into `group` may reallocate, so re-index by `newg` rather
> than holding the `NewG` reference; the C++ uses `group.back()`.) Reassign
> the moved states' group: walk the circular list starting at
> `NewG.first_state` (do/while until returning to head) setting
> `state[s].group = newg`.
> Update the agenda: if `g` is currently on the agenda
> (`agenda.contains(g)`), erase `g` and re-add both `g` and `newg` with
> their current sizes (`agenda.add(g, group[g].size)`,
> `agenda.add(newg, group[newg].size)`). Otherwise add only the smaller of
> the two: if `group[g].size < group[newg].size` add `g`, else add `newg`.
> Returns void.

> [spec:hfst:def:hopcroft.sfst.minimiser.state]
> class State {
>   Index group;
>   Index next_in_group;
>   Index previous_in_group;
>   Index first_transition;
> }

> [spec:hfst:def:hopcroft.sfst.minimiser.state-group]
> class StateGroup {
>   Index next;
>   Index next_in_agenda;
>   Index previous_in_agenda;
>   Index size;
>   Index first_state;
>   Index new_size;
>   Index first_new_state;
> }

> [spec:hfst:def:hopcroft.sfst.minimiser.state-group.init-fn]
> void init( Index i )

> [spec:hfst:sem:hopcroft.sfst.minimiser.state-group.init-fn]
> Initialises this StateGroup, given its own index `i`.
> Sets `next_in_agenda = i` (a self-loop, marking the group as not on the
> agenda); sets `size = new_size = 0`; sets `next = first_state =
> first_new_state = undef`. Note: `previous_in_agenda` is NOT set here. No
> return value.

> [spec:hfst:def:hopcroft.sfst.minimiser.state-group.is-empty-fn]
> bool is_empty()

> [spec:hfst:sem:hopcroft.sfst.minimiser.state-group.is-empty-fn]
> Returns whether the group's main state list is empty: returns
> `first_state == undef`.

> [spec:hfst:def:hopcroft.sfst.minimiser.state.state-fn]
> State()

> [spec:hfst:sem:hopcroft.sfst.minimiser.state.state-fn]
> Default constructor. Initialises all fields to `undef`:
> `group = next_in_group = previous_in_group = undef` and
> `first_transition = undef`.

> [spec:hfst:def:hopcroft.sfst.minimiser.transition]
> class Transition {
>   Index source;
>   Index next_for_target;
>   Index next_for_label;
>   Label label;
> }

> [spec:hfst:def:hopcroft.sfst.minimiser.transition.transition-fn]
> Transition( Index s, Label l, Index n )

> [spec:hfst:sem:hopcroft.sfst.minimiser.transition.transition-fn]
> Constructor. Given source state `s`, label `l`, and next-for-target index
> `n`: sets `source = s`; `label = l`; `next_for_target = n` (link to the
> previous head of the target state's incoming list); and
> `next_for_label = undef`.

