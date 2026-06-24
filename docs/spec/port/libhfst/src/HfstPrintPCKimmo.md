# libhfst/src/HfstPrintPCKimmo.cc

> [spec:hfst:def:hfst-print-pc-kimmo.hfst.print-pckimmo-fn]
> void

> [spec:hfst:sem:hfst-print-pc-kimmo.hfst.print-pckimmo-fn]
> `print_pckimmo(FILE* out, HfstTransducer& t)` writes a PC-KIMMO style transition
> table for transducer `t` to the C stdio stream `out`. It returns nothing (void).
>
> Step 1 — build a mutable working copy `mutt` of type `HfstBasicTransducer`,
> constructed from `t`. Initialize `HfstState s = 0` and `HfstState last = 0`,
> and an empty `std::set<std::pair<std::string,std::string>> pairs` (a set, so
> it is deduplicated and ordered by the default `pair<string,string>` ordering:
> lexicographic by first then second symbol).
>
> Step 2 — collect symbol pairs and count states. Iterate over every state in
> `mutt` (range-for over the transducer yields each state's transition list).
> For each arc (transition) in the state, read its input symbol (`first`) and
> output symbol (`second`) via `arc.get_input_symbol()` / `arc.get_output_symbol()`
> and insert the pair `(first, second)` into `pairs`. After processing each
> state, increment `last`. When the loop ends `last` equals the number of
> states.
>
> Step 3 — compute the first-column width `numwidth` (unsigned int, starts at 0):
> loop with `i` starting at 1, while `i < last` multiply `i` by 10 and increment
> `numwidth` each iteration. So `numwidth` is the number of decimal digits needed
> to represent `last` (e.g. 0 when last<=1, 1 when last in 2..10, 2 when last in
> 11..100, etc.; note the boundary uses `i < last`).
>
> Step 4 — print the input-symbol header row. Print a left-corner pad with
> `fprintf(out, "%*s  ", numwidth, " ")` (a space field right-padded to `numwidth`
> followed by two literal spaces). Then for each pair `p` in `pairs`, print one
> field with `fprintf(out, "%.*s ", numwidth, X)` where `X` is: the C-string "0"
> if `p.first == hfst::internal_epsilon`; the C-string "@" if `p.first ==
> hfst::internal_unknown`; otherwise `p.first.c_str()`. The `%.*s` precision is
> `numwidth`, so each symbol is truncated to at most `numwidth` characters,
> followed by a trailing space. Then print a newline.
>
> Step 5 — print the output-symbol header row identically to step 4 but keyed on
> `p.second`: print the same left-corner pad, then for each pair print "0" for
> `internal_epsilon`, "@" for `internal_unknown`, else `p.second.c_str()`, each
> with `%.*s ` (precision `numwidth`) and trailing space. Then print a newline.
>
> Step 6 — print one row per state. Iterate over every state in `mutt`. For the
> current state index `s`: if `mutt.is_final_state(s)` is true, print the row
> label `fprintf(out, "%.*d. ", numwidth, s + 1)` (1-based state number, a period,
> a space); otherwise print `fprintf(out, "%.*d: ", numwidth, s + 1)` (with a
> colon instead of period). Then build a `std::map<pair<string,string>,HfstState>
> transitions`: first initialize every pair in `pairs` to target `-1` (the sink
> placeholder). Then for each arc in the current state, set
> `transitions[(input,output)] = arc.get_target_state()`. Then iterate the map in
> its sorted key order and for each entry print `fprintf(out, "%.*d ", numwidth,
> trans.second + 1)` (target state + 1, so the sink `-1` prints as `0`), each with
> a trailing space. Print a newline. Increment `s`.
>
> Side effects: only writes to `out` via `fprintf`. No exceptions are thrown
> explicitly. The pair ordering used for the header columns and the per-state
> transition columns is the same (`pairs` is the master ordering; the per-row map
> is keyed on the same pairs), so columns align across all rows.

