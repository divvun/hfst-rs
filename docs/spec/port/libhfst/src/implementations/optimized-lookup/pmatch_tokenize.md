# libhfst/src/implementations/optimized-lookup/pmatch_tokenize.cc, libhfst/src/implementations/optimized-lookup/pmatch_tokenize.h

> [spec:hfst:def:pmatch-tokenize.find-first-not-of-def-fn]
> inline std::size_t find_first_not_of_def(const std::string & str, char c, std::size_t def)

> [spec:hfst:sem:pmatch-tokenize.find-first-not-of-def-fn]
> Returns the index of the first character in `str` that is not equal to the
> char `c` (i.e. `str.find_first_not_of(c)`). If no such character exists (the
> string is empty or consists entirely of `c`, so the underlying call returns
> `std::string::npos`), returns the fallback `def` instead. Pure, no side
> effects.

> [spec:hfst:def:pmatch-tokenize.find-last-not-of-def-fn]
> inline std::size_t find_last_not_of_def(const std::string & str, char c, std::size_t def)

> [spec:hfst:sem:pmatch-tokenize.find-last-not-of-def-fn]
> Returns the index of the last character in `str` that is not equal to the
> char `c` (i.e. `str.find_last_not_of(c)`). If no such character exists (the
> underlying call returns `std::string::npos`), returns the fallback `def`
> instead. Pure, no side effects.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.dedupe-locations-fn]
> const LocationVector

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.dedupe-locations-fn]
> Deduplicates a vector of `Location`s. If `s.dedupe` is false, returns
> `locations` unchanged. Otherwise:
> - If `s.print_weights` is true: insert all locations into a `std::set`
>   ordered by `location_compare` (which orders by weight, then tag, then
>   start, then length, then output) — duplicates per that comparator collapse —
>   then copy the set's contents (already sorted by that ordering) into a new
>   `LocationVector` and return it.
> - If `s.print_weights` is false: insert all locations into a `std::set`
>   ordered by `location_compare_ignoring_weights` (same ordering but weight
>   excluded), copy to a `LocationVector`, then re-sort that vector with
>   `std::sort` using `location_compare_using_only_weights` (ascending by
>   weight only), and return it.
> Returns a new vector; does not mutate the input.

> PORT DIVERGENCE (deliberate, `s.dedupe` default): upstream defaults `dedupe`
> false and exposes it as `-u`. This port defaults it TRUE. Two readings
> identical in span, output, tag and weight are two paths through the network
> projecting to a single analysis; a CG rule cannot act on one differently from
> the other, so the multiplicity carries no information a grammar can use. On
> lang-sma's tokeniser over 2000 corpus lines that is 1198 repeated readings in
> `-c` output. `--duplicates` (long-only, a port addition) restores upstream's
> behaviour and is byte-identical to hfst-tokenize.
>
> The dedup lives HERE, consulted per call from `match_and_print`, and NOT in
> `locate()`. An earlier revision deduped inside `locate()`, which put it on the
> shared path so it silently reached `giellacg` and `visl` as well — both of
> which set `dedupe` themselves, so the effect was invisible there and only
> `cg` showed it. Library callers of `locate()` still see every path.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.empty-to-underscore-fn]
> std::string

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.empty-to-underscore-fn]
> If the input string `to_test` is empty (size 0), returns the literal `"_"`;
> otherwise returns `to_test` unchanged. Pure.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.fetch-and-kill-between-fn]
> std::string

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.fetch-and-kill-between-fn]
> Extracts and removes a delimited substring from `analysis` (passed by
> reference, mutated in place). Finds `start` = first position of `left` in
> `analysis`, then `stop` = first position of `right` searched from `start + 1`.
> If either is `npos`, returns `""` (and does not modify `analysis`). Otherwise
> the return value is the substring of `analysis` lying strictly between the
> `left` and `right` markers: `analysis.substr(start + left.size(), stop - start
> - left.size())`. Then erases from `analysis` the whole span from `start`
> through the end of the `right` marker (length `stop - start + right.size()`),
> removing both markers and the content. Returns the extracted inner text.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.fetch-and-kill-feats-fn]
> std::string

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.fetch-and-kill-feats-fn]
> Builds a CoNLL-U FEATS string by extracting morphological features from
> `analysis` (passed by reference, mutated in place). For each feature, calls
> `fetch_and_kill_between` with the bracketed marker and `"]"`, which removes the
> matched span from `analysis` and returns the inner value `tmp`. If `tmp` is
> non-empty, appends `"<Name>=<tmp>|"` to the result accumulator `retval`. The
> markers and output names are processed in this fixed order:
> `[ANIMACY=]`→Animacy, `[ASPECT=]`→Aspect, `[CASE=]`→Case, `[DEFINITE=]`→
> Definite, `[CMP=]`→Degree, `[GENDER=]`→Gender, `[MOOD=]`→Mood, `[NEGATIVE=]`→
> Negative, `[NUMTYPE=]`→Numtype, `[NUM=]`→Number, `[PERS=]`→Person, `[POSS=]`→
> Poss, `[PRONTYPE=]`→PronType, `[REFLEX=]`→Reflex, `[TENSE=]`→Tense,
> `[VERBFORM=]`→VerbForm, `[VOICE=]`→Voice. After all features, if `retval` is
> non-empty, erase its trailing `|` (the last character). Returns the
> pipe-joined `Name=Value` string (empty if no features matched). Side effect:
> all matched feature spans are removed from `analysis`.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.is-cg-tag-fn]
> bool

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.is-cg-tag-fn]
> Decides whether a symbol string `str` is a tag (non-lemma), defined as being
> strictly longer than its first grapheme "character" rather than a single
> character. Uses ICU: constructs a `UnicodeString` from `str`, sets it as the
> text of the shared `characterBoundary` BreakIterator, and computes `i_after`
> = `characterBoundary->following(0)`, the byte/code-unit index just past the
> first character boundary. Then:
> - If the codepoint at `i_after` (`us.char32At(i_after)`) is a Unicode
>   modifier letter (`U_MODIFIER_LETTER`): advance one more character boundary
>   via `characterBoundary->following(i_after)`, and set `is_tag` =
>   `us.length() > ` that next boundary (i.e. there is content beyond the base
>   character plus its trailing modifier). If `is_tag` is false and the global
>   `IS_CG_TAG_MODIFIER_WARNED` flag has not yet been set, print a warning to
>   stderr about skipping a modifier letter for the baseform and set the flag
>   true (warn only once). Return `is_tag`.
> - Otherwise (first char is not a modifier letter): return `us.length() >
>   i_after`.
> Reads/writes the global one-time-warning flag and uses the shared
> `characterBoundary` iterator state.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn]
> bool

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn]
> PORT ADDITION. True when a `Location` segments a token without analysing it:
> its `output` is empty, its `output` contains the `" ??"` unknown marker a
> pmatch script can emit, or its `weight` is at least
> `pmatch-tokenize.hfst-ol-tokenize.unanalysed-weight`. Upstream spells the
> first two conditions inline at each of the four sites that care
> (`keep_n_best_weight`, `locate_fullmatch`, `print_reading_giellacg`,
> `print_location_vector_giellacg`) and has no third condition, because
> upstream's fallback reading is unreachable. Naming the predicate once keeps
> those sites in agreement about what an unanalysed reading is.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.unanalysed-weight]
> const Weight

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.unanalysed-weight]
> PORT ADDITION. The weight `make_naive_tokenizer` puts on its `others`
> fallback: `INFINITE_WEIGHT`, i.e. `NO_TABLE_INDEX` cast to `Weight`.
>
> PORT DIVERGENCE (upstream data loss deliberately fixed). Upstream weights the
> fallback with `std::numeric_limits<float>::max()`. But `get_analyses`
> abandons any walk whose running weight exceeds the `weight_cutoff` handed to
> `locate`, every caller in this module passes `INFINITE_WEIGHT`, and
> `INFINITE_WEIGHT` is about 4.29e9 — twenty-nine orders of magnitude below
> `float` max. The fallback branch is therefore pruned before it can accept,
> the `others` arm never fires, and a run of dictionary-external characters
> falls through to the `"@_NONMATCHING_@"` path, which the default output mode
> discards: `dogs cot cats` against a plain `.hfstol` prints `dogs\ncats`. C++
> `hfst-tokenize` does the same on the same fixture; `[dec:hfst:independent-fork]`
> makes a faithfully ported upstream bug this port's bug, and a tokenizer that
> discards input is a data-loss bug. `INFINITE_WEIGHT` is the largest weight
> the runtime admits (its cutoff test is a strict `>`), so it preserves
> upstream's intent — the fallback loses to any realistic dictionary path —
> while staying reachable.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.keep-n-best-weight-fn]
> const LocationVector

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.keep-n-best-weight-fn]
> Keeps only the `s.max_weight_classes` best (lowest) weight classes from
> `locations`, assuming `locations` is already ordered by weight. If
> `locations.size() <= s.max_weight_classes`, returns `locations` unchanged (no
> copy needed). Otherwise iterates the input in order, accumulating kept entries
> into `goodweight`:
> - Any location with empty `output` is always pushed to `goodweight` and skipped
>   (does not count toward weight classes).
> - For each location with non-empty output, read its `weight` as
>   `current_weight`. Track `classes_found` (init -1) and `last_weight_class`. On
>   the first such location set `classes_found = 1` and `last_weight_class =
>   current_weight`. On subsequent ones, if `current_weight` differs from
>   `last_weight_class`, update `last_weight_class` and increment `classes_found`
>   (a new weight class).
> - If `classes_found > s.max_weight_classes`, break out of the loop; otherwise
>   push the location to `goodweight`.
> Returns `goodweight`. Pure with respect to inputs (builds a new vector).
>
> PORT DIVERGENCE. The pass-through test is
> `pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn`, which covers the ` ??`
> unknown marker (issue #562) and the naive tokenizer's fallback weight as well
> as the empty output upstream tests for. Since the fallback sits at the worst
> weight rather than the best, counting it as a weight class would let
> `--weight-classes n` spend one of its n classes on a reading that is only a
> placeholder.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.locate-fullmatch-fn]
> const LocationVector

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.locate-fullmatch-fn]
> Looks up `form` in `container`, keeping only complete, meaningful analyses.
> Calls `container.locate(form, s.time_cutoff)` to get `sublocs` (a
> `LocationVectorVector`). Iterates each inner `LocationVector` `it`; skips it
> (continue) if any of: it is empty; it has exactly one element whose output is
> `"@_NONMATCHING_@"`; or its first element's `input.length()` does not equal
> `form.length()` (i.e. it does not cover the whole form). For surviving inner
> vectors, compute `loc = keep_n_best_weight(dedupe_locations(*it, s), s)`. Then
> for each location in `loc`, keep it only if its `output` is non-empty, its
> `weight` is strictly less than `std::numeric_limits<float>::max()`, and its
> output does not contain the substring `" ??"`. For each kept location, if
> `s.hack_uncompose` is set call `container.uncompose(*loc_it)` to mutate it, then
> push it onto the result `loc_filtered`. Returns `loc_filtered`.
>
> PORT DIVERGENCE. The three-part keep test is
> `!pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn`, which says the same
> thing about the two conditions upstream can actually observe and additionally
> excludes the naive tokenizer's fallback reading. Upstream's `float` max test
> was written for that reading (its comment asks why the `<W:inf>` are not
> excluded earlier) but could never fire, since the reading it names is pruned
> by the weight cutoff before it reaches here.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.location-compare-fn]
> bool

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.location-compare-fn]
> Strict-weak-ordering comparator for two `Location`s. Compares lexicographically
> in this field order, returning true if `lhs` sorts before `rhs`: `weight`, then
> (if weights equal) `tag`, then (if tags equal) `start`, then (if starts equal)
> `length`, then (if lengths equal) `output`. Each tier uses `<`. Pure.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.location-compare-ignoring-weights-fn]
> bool

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.location-compare-ignoring-weights-fn]
> Strict-weak-ordering comparator identical to `location_compare` but omitting
> the `weight` tier entirely. Returns true if `lhs` sorts before `rhs` comparing
> lexicographically by `tag`, then `start`, then `length`, then `output`, each
> using `<`. Pure.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.location-compare-using-only-weights-fn]
> bool

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.location-compare-using-only-weights-fn]
> Comparator returning `lhs.weight < rhs.weight` — orders two `Location`s purely
> by ascending weight. Pure.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.match-and-print-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.match-and-print-fn]
> Tokenizes/analyzes one line of `input_text` and prints results. Calls
> `container.locate(input_text, s.time_cutoff)` to obtain `locations` (a
> `LocationVectorVector`). If `locations` is empty and `s.print_all` is set, calls
> `print_no_output(input_text, outstream, s)` and returns. Otherwise initializes
> `token_number = 1` and iterates each inner `LocationVector` `it`:
> - If `it` is a single element whose output equals `"@_NONMATCHING_@"`: if
>   `s.print_all`, call `print_nonmatching_sequence(it->at(0).input, ...)`; then
>   `continue` (token_number not incremented for nonmatching cohorts).
> - Otherwise call `print_location_vector(container,
>   keep_n_best_weight(dedupe_locations(*it, s), s), outstream, token_number, s)`
>   and increment `token_number`.
> After the loop, if `s.output_format` is `finnpos`, `tokenize`, or `xerox`,
> print an extra blank line (`std::endl`) to `outstream`.
>
> PORT DIVERGENCE (upstream data loss deliberately fixed). Once the naive
> tokenizer's `others` fallback is reachable (see
> `pmatch-tokenize.hfst-ol-tokenize.unanalysed-weight`) it covers every token,
> not only the ones the dictionary cannot analyse, because it accepts any run
> of non-boundary characters. So this port partitions
> `keep_n_best_weight(dedupe_locations(*it, s), s)` on
> `pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn`. If any analysed reading
> survives, only those are passed to `print_location_vector` — the placeholder
> never appears beside a real analysis. If none does, the whole (unpartitioned)
> vector goes to `pmatch-tokenize.hfst-ol-tokenize.print-unanalysed-location-fn`
> instead. `token_number` still advances in both cases: an unanalysed token is
> a token.
>
> Nonmatching cohorts are unchanged and still print only under `s.print_all`.
> They are now genuinely separator material — with the fallback firing, the
> unmatched *word* is a token of its own rather than one blob glued to the
> whitespace on either side of it — so the default stream gains no whitespace
> lines, and `-a` still reconstructs its input verbatim.
>
> The partition keys on the fallback's WEIGHT, not on `is_unanalysed`, and that
> distinction is load-bearing. A pmatch script emits its own `" ??"` unknown
> marker — lang-sma's tokeniser puts `"Manne" ??` beside the real reading
> `manne Pron Pers Sg1 Nom` — and upstream prints those in plain `cg` while
> `print_reading_giellacg` drops them at indent 1. Both behaviours are kept.
>
> An earlier revision filtered on the marker instead, which made plain `cg`
> agree with `giellacg` and removed 778,856 readings over lang-sma's free
> corpus. Those readings are the script's own output, not an artifact of the
> fallback, and reshaping a stream a grammar parses is outside what this change
> is for. Verified byte-identical to hfst-tokenize 3.17.1 in both `-c` and `-g`
> over that tokeniser, for analysed input and for an unknown word alike.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-unanalysed-location-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-unanalysed-location-fn]
> PORT ADDITION. Prints a token that was segmented but not analysed, using the
> marking the requested format already reserves for unknown material, so that a
> downstream parser cannot read the placeholder as an analysis. Dispatch on
> `s.output_format`:
> - `cg` and `xerox`: `print_no_output(locations[0].input, outstream, s)` —
>   `"<w>"\n\t"w" ?` and `w\tw+?\tinf` respectively, each with the trailing
>   blank line that separates cohorts in those formats.
> - `tokenize` and `space_separated`: the input alone, since neither has an
>   analysis column; then, if `s.print_weights`, a tab and the literal `inf`
>   (the weight xerox prints for the same condition) rather than the numeric
>   sentinel. Terminated by a newline for `tokenize` and a space for
>   `space_separated`, matching `print_location_vector`.
> - `giellacg`, `visl`, `conllu`, `finnpos`: `print_location_vector`, which
>   already renders the condition — `print_location_vector_giellacg` has an
>   unknown-but-tokenised branch printing `"w" ?`, and the other two render a
>   reading with no lemma or features as underscores.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.output-format]
> enum OutputFormat {
>   tokenize;
>   space_separated;
>   xerox;
>   cg;
>   finnpos;
>   giellacg;
>   conllu;
>   visl;
> }

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-cg-subreading-ex-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-cg-subreading-ex-fn]
> Prints one CG subreading line. Identical to `print_cg_subreading` (lemma/tag
> quoting of output symbols `[out_beg, out_end)`, weight tag, and trailing
> wordform from input symbols `[in_beg, in_end)`), with one extra step plus an
> additional `middle` parameter: after closing the lemma quotes (and before the
> weight tag), if `s.hack_uncompose` is true and `middle` is non-empty, print
> ` "<middle>"MIDTAPE`. Specifically:
> - Emit `indent` tab characters.
> - For each output symbol in `[out_beg, out_end)`: skip `"@PMATCH_BACKTRACK@"`;
>   compute `is_tag = is_cg_tag(sym)`; on transition from inside-lemma to a tag,
>   close with `"`; on transition from outside-lemma to a non-tag, open with `"`;
>   then print the symbol via `print_escaping_backslashes`. After the loop, if
>   still in_lemma, emit a closing `"`.
> - If `s.hack_uncompose` and `!middle.empty()`: emit ` "middle"MIDTAPE`.
> - If `s.print_weights`: format `weight` with `std::fixed`/setprecision(9), then
>   trim trailing zeros after the decimal point (keeping one zero), and emit
>   ` <W:<rounded>>` (using the `wtag` "W").
> - If `in_beg != in_end`: concatenate the input symbols into a form string and
>   emit ` "<` + escaped form + `>"`.
> - End the line with `std::endl`. Writes to `outstream` only.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-cg-subreading-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-cg-subreading-fn]
> Prints one CG subreading line to `outstream`. Steps:
> - Emit `indent` tab characters (`string(indent, '\t')`).
> - Iterate output symbols in `[out_beg, out_end)` with an `in_lemma` flag
>   (init false): skip any symbol equal to `"@PMATCH_BACKTRACK@"`; compute
>   `is_tag = is_cg_tag(sym)`. If currently in_lemma and the symbol is a tag,
>   leave the lemma (set in_lemma false) and emit a closing `"`. If currently
>   not in_lemma and the symbol is not a tag, enter the lemma (set in_lemma
>   true) and emit an opening `"`. Then print the symbol via
>   `print_escaping_backslashes`. After the loop, if still in_lemma emit a
>   closing `"`.
> - If `s.print_weights`: format `weight` with `std::fixed` and precision 9 into
>   a string, then trim trailing zeros after the decimal point (scanning from
>   the right, keeping exactly one zero immediately after the dot, stopping at
>   the `.`); emit ` <W:<rounded>>` using `wtag` ("W").
> - If `in_beg != in_end`: concatenate input symbols `[in_beg, in_end)` into a
>   form string, and emit ` "<` + (escaped form) + `>"`.
> - Emit `std::endl` to finish the line. Writes only to `outstream`.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-escaping-backslashes-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-escaping-backslashes-fn]
> Writes `str` to `outstream`, doubling every backslash. Uses two indices `i=0,
> j=0`; while `j = str.find("\\", i)` is not `npos`, emit `str.substr(i, j-i)`
> (the run before the backslash) followed by `"\\\\"` (two literal backslashes),
> then set `i = j + 1`. After the loop, emit the trailing remainder
> `str.substr(i, j-i)` (where `j` is `npos`, so this prints the rest of the
> string from `i`). Writes only to `outstream`.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-escaping-newlines-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-escaping-newlines-fn]
> Writes `str` to `outstream`, replacing newline characters with escape
> sequences. Uses indices `i=0, j=0`; while `j = str.find_first_of("\n\r", i)`
> is not `npos`, emit `str.substr(i, j-i)` (the run before the line break), then
> if `str[j] == '\n'` emit `"\\n"`, else if `str[j] == '\r'` emit `"\\r"`; set
> `i = j + 1`. After the loop emit the trailing remainder `str.substr(i, j-i)`
> (with `j == npos`, prints the rest from `i`). Writes only to `outstream`.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-location-vector-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-location-vector-fn]
> Prints one cohort of `locations` to `outstream` according to
> `s.output_format`. Branches (all except xerox/conllu/finnpos require
> `locations.size() != 0`):
> - `tokenize`: print `locations[0].input`; if `s.print_weights` append
>   `"\t<weight>"`; then `endl`. If `locations[0].tag == "<Boundary=Sentence>"`
>   print an extra blank line.
> - `space_separated`: print `locations[0].input`; if print_weights append
>   `"\t<weight>"`; print a space (no newline). If first tag is the sentence
>   boundary, print a blank line.
> - `cg`: print cohort header `"<` + escaped `locations[0].input` + `>"` + endl.
>   For each location: if `output.find(input) == 0` (output begins with input),
>   print `\t"` + escaped input + `"` + `output.substr(input.size())`; else
>   print `\t` + `output`. If print_weights append `"\t<weight>"`; then endl.
>   Finally print a blank line.
>
>   Every location is printed, including ones byte-identical to an earlier one:
>   a single accepting configuration reachable through several structurally
>   distinct paths yields one location per path, and the resulting multiplicity
>   of a reading inside a cohort is part of the Constraint Grammar contract.
>   (Upstream offers an opt-in `-u` unique flag; it is not applied by default,
>   and the location vector is never filtered at this site.)
>
>   `<weight>` is the RAW weight written to the output stream, which
>   `hfst-tokenize`'s `process_input` has put into `std::fixed` with precision 10
>   for this format: the column reads e.g. `0.0000000000`, not `0`. This is the
>   only cg/giellacg/visl print path that emits an unformatted weight — the
>   giellacg subreading printers set their own precision (9) and trim — so a port
>   that does not carry a stream-wide mode must apply fixed-10 here.
> - `giellacg` or `visl`: delegate to `print_location_vector_giellacg(container,
>   locations, outstream, s)`.
> - `xerox`: compute `best_weight` = min weight over locations. With a
>   `printed_something` flag, for each location: print it only if (`s.beam < 0`
>   or `weight <= best_weight + s.beam`) AND (`output != input`, i.e. it has a
>   real analysis, OR it is the last location and nothing has been printed yet).
>   When printing, emit `input` + `\t` + `output`; if print_weights, emit
>   `\t<best_weight>` when it is the last-and-first-printed fallback else
>   `\t<weight>`; then endl and set printed_something. After the loop, if first
>   tag is sentence boundary print a blank line, then always print a final blank
>   line.
> - `conllu`: find `best_location` = the location with the lowest weight
>   (`INFINITE_WEIGHT` initial). Print `token_number`, then `\t` +
>   `best_location.input`, then tab-separated CoNLL-U columns each wrapped in
>   `empty_to_underscore`: LEMMA via `fetch_and_kill_between("[WORD_ID=","]",
>   output)`, UPOS via `[UPOS=]`, XPOS via `[XPOS=]`, FEATS via
>   `fetch_and_kill_feats(output)` (these mutate `best_location.output`), then
>   literal `_` for HEAD, DEPREL, DEPS, then MISC = `empty_to_underscore(output)`
>   (the remaining output after extraction). If print_weights append
>   `\t<weight>`; then endl.
> - `finnpos`: collect a set of `lemmas` and a set of `tags`. For each location,
>   split its `output` at the last space: `lemma` = part before, `tag` = part
>   after; insert lemma into `lemmas` only if it contains no space, and tag into
>   `tags` only if it contains no space (skip the whole location if there is no
>   space at all). Print `locations[0].input` + `\t_\t`. Then print the lemmas:
>   `_` if empty, else the lemmas space-joined (built by appending each + space
>   and trimming the final char). Print `\t`, then the tags similarly (`_` or
>   space-joined). Print `\t_` + endl. If first tag is the sentence boundary
>   print an extra blank line.
> Writes only to `outstream`; the conllu branch mutates a local copy of the
> best location's output via the fetch-and-kill calls.
>
> PORT DIVERGENCE. The conllu branch requires `locations.size() != 0` like the
> others, and seeds `best_location` from `locations[0]` rather than from a
> default-constructed `Location`, scanning from the second element. Upstream's
> seed is beaten only by a weight strictly below `INFINITE_WEIGHT`, so a cohort
> whose only reading weighs exactly that — the naive tokenizer's unanalysed
> token — kept the default and blanked the FORM column. Seeding from the first
> reading is otherwise identical: the input is already ordered by ascending
> weight and the strict `<` still keeps the first of an equally weighted run.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-location-vector-giellacg-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-location-vector-giellacg-fn]
> Prints a CG cohort for `locations` in giellacg/visl style, handling
> backtracking. Steps:
> - Print the cohort header `"<` + escaped `locations[0].input` + `>"` + endl.
> - If there is exactly one location and it is unanalysed, treat it as
>   unknown-but-tokenized: print `\t"` + escaped input + `" ?` + endl and
>   return. (PORT DIVERGENCE: the test is
>   `pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn`, so the naive
>   tokenizer's fallback reading gets the same `"w" ?` cohort as the empty and
>   `" ??"` outputs upstream tests for, instead of a bare reading a CG grammar
>   would read as an analysis.)
> - Print regular analyses: for each location, copy it into a heap `Location`
>   (`hack`); if `s.hack_uncompose`, call `container.uncompose(*hack)`; call
>   `print_reading_giellacg(hack, 1, false, outstream, s)` and take the returned
>   `SplitPoints` `.first`; if non-empty insert it into a `std::set<SplitPoints>`
>   `backtrack`. Delete `hack`. If `backtrack` is empty, return.
> - Backtracking handling: let `in_syms = locations[0].input_symbol_strings`.
>   For each `bt_points` set in `backtrack`: split `in_syms` via
>   `split_at(in_syms, &bt_points)` into `words` (one substring per inter-point
>   span). For each word, trim leading/trailing spaces using
>   `find_first_not_of_def`/`find_last_not_of_def` to compute `first`/`last`, form
>   the trimmed `form`, and look it up via `locate_fullmatch(container, form, s)`
>   into `loc`. If `loc` is empty and `s.verbose`, print a warning to stderr
>   about a backtracking substring with no analyses (but still proceed). If the
>   trimmed form is shorter than the original word, re-insert the stripped spaces
>   into each result location: build `lspace` (`first` spaces) and `rspace`
>   (`it->length() - last` spaces); for each location in `loc` set its `input =
>   form`, prepend `lspace` and append `rspace` to `input_symbol_strings`, and
>   add `first` to every entry of `input_parts`. Push `loc` onto `splitlocs`.
>   If `splitlocs` ends up empty, continue to the next bt_points.
> - Then reorder/emit the split readings as a non-branching CG cohort by calling
>   `print_splitlocs_r(outstream, splitlocs, bottom = splitlocs.size()-1, depth =
>   0, indent = 1, out, s)`, where `out` is a vector of `splitlocs.size()`
>   ostringstreams. (The CG convention: the last word is least indented.)
> Writes to `outstream` and stderr; allocates and frees temporary `Location`
> objects.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-no-output-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-no-output-fn]
> Prints `input` formatted as an unanalyzed token for the chosen format:
> - `tokenize` or `space_separated`: print `input`.
> - `xerox`: print `input` + `\t` + `input` + `+?`.
> - `cg` or `giellacg`: print `"<`, then `input` via `print_escaping_backslashes`,
>   then `>"` + endl + `\t"`, then `input` again escaped, then `" ?`.
> After the format-specific output, always print `"\n\n"` (two newlines). Writes
> only to `outstream`.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-nonmatching-sequence-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-nonmatching-sequence-fn]
> Prints a non-matching sequence `str` formatted per `s.output_format`:
> - `tokenize` or `space_separated`: print `str`.
> - `xerox`: print `str` + `\t` + `str` + `+?`.
> - `cg`: print `"<`, escaped `str`, `>"` + endl + `\t"`, escaped `str`, `" ?`.
> - `giellacg`: print `:` then `str` via `print_escaping_newlines`.
> - `visl`: print `str`.
> - `conllu`: print `str`.
> - `finnpos`: print `str` + `\t_\t_\t_\t_`.
> After the format-specific output, always print a single `"\n"`. Writes only to
> `outstream`.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-reading-giellacg-fn]
> pair<SplitPoints, size_t>

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-reading-giellacg-fn]
> Prints one analysis `loc` as a (possibly multi-level) giellacg reading,
> peeling sub-readings and input marks from the right, and returns the set of
> backtracking split points plus the final indent. Returns immediately with
> `(empty SplitPoints, indent)` if `loc->output` is empty, or if `loc` is
> unanalysed and `indent == 1`. (PORT DIVERGENCE: the second test is
> `pmatch-tokenize.hfst-ol-tokenize.is-unanalysed-fn` rather than the `" ??"`
> substring alone, so a naive-tokenizer fallback reading is suppressed at the
> top level of a cohort that also has a real analysis.)
> Sets up output iterators over `loc->output_symbol_strings`
> (`out_beg`..`out_end`) and input iterators over `loc->input_symbol_strings`
> (`in_beg`..`in_end`). If `!always_wftag`, suppress the input wordform tag by
> setting `in_beg = in_end` (it gets restored later only if an input mark is
> seen). Let `part = loc->input_parts.size()`. Then loop:
> - Compute `out_part = part>0 ? loc->output_parts[part-1] : 0`. While the output
>   symbol just before `out_part` is `"@PMATCH_BACKTRACK@"`, record a backtrack
>   point by inserting `loc->input_parts[part-1]` into `bt_its`, decrement
>   `part`, and recompute `out_part`.
> - Scan from `out_end-1` leftward down to just past `out_part` looking for the
>   subreading separator `"#"`. If found at position `it`, set `out_beg = it+1`
>   and mark `sub_found = true`, break.
> - If no subreading separator was found: if `out_part > 0` there is an input
>   mark — set `out_beg = output_begin + out_part`, set `in_beg = input_begin +
>   loc->input_parts[part-1]`, and decrement `part`; else (no marks left) set
>   `out_beg = output_begin`, and if `in_end` is not the true input end (we have
>   already seen an input mark) set `in_beg = input_begin` to flush the rest.
> - Print the current level via `print_cg_subreading_ex(indent, out_beg, out_end,
>   loc->weight, in_beg, in_end, loc->middle, outstream, s)`.
> - If `out_beg` reached the beginning of the output symbols, break. Otherwise
>   increment `indent`, set `out_end = out_beg` and `in_end = in_beg`, and if a
>   subreading separator was used decrement `out_end` to skip that separator
>   symbol; loop again.
> After the loop, if `bt_its` is non-empty, also insert `0` and
> `loc->input_symbol_strings.size()` (the two endpoints) into it. Return
> `make_pair(bt_its, indent)`. Writes to `outstream`.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.print-splitlocs-r-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.print-splitlocs-r-fn]
> Recursively emits the cartesian product of the split-location analyses as
> non-branching CG cohorts. Parameters: `splitlocs` (vector of LocationVectors,
> one per split word), `bottom` (= last word index), `depth` (current recursion
> depth), `indent`, a scratch vector `out` of ostringstreams (one per depth), and
> settings `s`.
> - Select `locs = splitlocs.at(bottom - depth)` (CG indents the last word least,
>   so depth 0 corresponds to the bottom/last word).
> - For each `loc` in `locs`: clear `out.at(depth)` (reset its contents), then
>   render this reading into it via `print_reading_giellacg(&loc, indent, true,
>   out.at(depth), s)`.
>   - If `depth == bottom` (we have a full assignment for every word), flush:
>     iterate `out` front-to-back and write each buffer's string to `outstream`.
>   - Otherwise recurse: `print_splitlocs_r(outstream, splitlocs, bottom, depth+1,
>     indent+1, out, s)`.
> Writes to `outstream`; mutates the shared `out` buffers as scratch space.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.process-input-fn]
> void

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.process-input-fn]
> Drives the whole tokenization over an input stream. First calls
> `container.set_single_codepoint_tokenization(!s.tokenize_multichar)` (single
> codepoint tokenization is on unless multichar tokenization is requested). Then
> reads `instream` line by line using a fixed 4096-byte buffer via
> `instream.getline(line, 4096)`. For each line, constructs `input_text` from it,
> and if non-empty calls `match_and_print(container, outstream, input_text, s)`.
> Loops until getline fails (EOF/error). Writes to `outstream` via
> `match_and_print`. Note: lines longer than the buffer are handled by getline's
> standard behavior (truncation/failbit per the buffer size).

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.split-at-fn]
> const hfst::StringVector

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.split-at-fn]
> Treats `syms` (a StringVector) as a sequence of "characters" and splits it at
> the indices in `splitpoints` (an ordered `std::set<size_t>` assumed to include
> both endpoints 0 and `syms.size()`). If `splitpoints->size() < 2`, prints
> `"split_at called with "` to stderr and returns an empty StringVector.
> Otherwise iterates consecutive pairs of split points (from begin up to the
> next-to-last): for each pair `(*it, *next(it))`, concatenates the symbols in
> the half-open range `syms[*it .. *next(it))` into a single string (joined with
> no separator) and pushes that string onto the result `subs`. Returns `subs`
> (one string per inter-point span). May write to stderr.

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.split-points]
> typedef std::set<size_t> SplitPoints

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.tokenize-settings]
> struct TokenizeSettings {
>   OutputFormat output_format = tokenize;
>   int max_weight_classes = std::numeric_limits<int>::max();
>   bool dedupe = false;
>   bool print_weights = false;
>   bool print_all = false;
>   double time_cutoff = 0.0;
>   float weight_cutoff = -1.0;
>   bool verbose = true;
>   float beam = -1.0;
>   bool tokenize_multichar = false;
>   bool hack_uncompose = false;
> }

> [spec:hfst:def:pmatch-tokenize.hfst-ol-tokenize.u8-first-codepoint-size-fn]
> size_t

> [spec:hfst:sem:pmatch-tokenize.hfst-ol-tokenize.u8-first-codepoint-size-fn]
> Returns the byte length of the first UTF-8 codepoint pointed to by `c` (an
> unsigned char pointer), based on the lead byte `*c`:
> - `*c <= 127` (ASCII): returns 1.
> - lead bits `11110xxx` (`(*c & 0xF0) == 0xF0`): returns 4.
> - lead bits `1110xxxx` (`(*c & 0xE0) == 0xE0`): returns 3.
> - lead bits `110xxxxx` (`(*c & 0xC0) == 0xC0`): returns 2.
> - otherwise (a stray continuation byte / invalid lead): returns 0.
> Note the checks are in this order, so the more-significant masks are tested
> first. Pure; reads one byte.

