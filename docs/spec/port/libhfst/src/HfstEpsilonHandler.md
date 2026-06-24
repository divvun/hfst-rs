# libhfst/src/HfstEpsilonHandler.cc, libhfst/src/HfstEpsilonHandler.h

> [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler]
> class HfstEpsilonHandler {
>   HfstStateVector epsilon_path;
>   size_t max_cycles;
>   size_t cycles;
> }

> [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.can-continue-fn]
> bool HfstEpsilonHandler::can_continue(hfst::implementations::HfstState s)

> [spec:hfst:sem:hfst-epsilon-handler.hfst.hfst-epsilon-handler.can-continue-fn]
> Called at the start of lookup_fd to decide whether traversal may proceed
> into state `s`, updating the cycle counter and epsilon path. Iterate over
> `epsilon_path` from the front. If an element equal to `s` is found, a cycle
> is detected: advance the iterator one position past the matched element,
> then erase the range from that advanced position through the end of
> `epsilon_path` (i.e. keep everything up to and including the matched
> occurrence of `s`, drop everything after it), increment `cycles` by 1, and
> if `cycles` now exceeds `max_cycles` return false, otherwise return true.
> If the loop finishes without finding `s` (no cycle), return true. Mutates
> `epsilon_path` (truncation) and `cycles` only when a cycle is found.

> [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.hfst-epsilon-handler-fn]
> HfstEpsilonHandler::HfstEpsilonHandler(size_t cutoff)

> [spec:hfst:sem:hfst-epsilon-handler.hfst.hfst-epsilon-handler.hfst-epsilon-handler-fn]
> Constructor. Initializes `epsilon_path` to an empty vector, sets
> `max_cycles` to the `cutoff` argument, and sets `cycles` to 0. The cutoff
> is the maximum number of consecutive input-epsilon cycles allowed (the
> cycles need not be identical).

> [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.hfst-state-vector]
> typedef std::vector<hfst::implementations::HfstState> HfstStateVector

> [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.pop-back-fn]
> void HfstEpsilonHandler::pop_back()

> [spec:hfst:sem:hfst-epsilon-handler.hfst.hfst-epsilon-handler.pop-back-fn]
> Removes the last state from `epsilon_path`. If `epsilon_path` is non-empty,
> pop its last element; if it is empty, do nothing. Returns nothing.

> [spec:hfst:def:hfst-epsilon-handler.hfst.hfst-epsilon-handler.push-back-fn]
> void HfstEpsilonHandler::push_back(hfst::implementations::HfstState s)

> [spec:hfst:sem:hfst-epsilon-handler.hfst.hfst-epsilon-handler.push-back-fn]
> Called before recursing into lookup_fd; appends state `s` to
> `epsilon_path` only if it is not already the last element. If
> `epsilon_path` is non-empty, push `s` only when its current back element is
> not equal to `s` (avoiding consecutive duplicates). If `epsilon_path` is
> empty, push `s` unconditionally. Returns nothing.

