# back-ends/sfst/compact.cc, back-ends/sfst/compact.h

> [spec:hfst:def:compact.sfst.c-analysis]
> typedef std::vector<unsigned int> CAnalysis

> [spec:hfst:def:compact.sfst.compact-transducer]
> class CompactTransducer {
>   unsigned int number_of_nodes;
>   char *finalp;
>   unsigned int *first_arc;
>   unsigned int number_of_arcs;
>   Label *label;
>   unsigned int *target_node;
>   float *final_logprob;
>   float *arc_logprob;
>   bool both_layers;
>   bool simplest_only;
>   Alphabet alphabet;
> }

> [spec:hfst:def:compact.sfst.compact-transducer.analyze-fn]
> void CompactTransducer::analyze(unsigned int n, vector<Character> &input,

> [spec:hfst:sem:compact.sfst.compact-transducer.analyze-fn]
> Recursive depth-first analysis of `input` (a vector of Character codes)
> starting from transducer node `n` at input position `ipos`, accumulating the
> current partial analysis (a vector of arc indices) in `ca` and collecting
> complete analyses into `analyses`.
> Step by step:
> - If `analyses.size() > 10000`, return immediately (caps the number of
>   analyses produced).
> - If node `n` is final (`finalp[n]` non-zero) and the whole input has been
>   consumed (`ipos == input.size()`), push a copy of `ca` onto `analyses`.
> - Iterate over the outgoing arcs of node `n`. Node `n`'s arcs occupy indices
>   `first_arc[n] .. first_arc[n+1]-1` (arcs are sorted so epsilon-upper arcs
>   come first). For each arc `i` starting at `first_arc[n]` while
>   `i < first_arc[n+1]` and `label[i].upper_char() == Label::epsilon`: push `i`
>   onto `ca`, recurse with `analyze(target_node[i], input, ipos, ca, analyses)`
>   keeping the same `ipos` (epsilon consumes no input), then pop `i` off `ca`.
>   Variable `i` retains the index of the first non-epsilon arc afterwards.
> - If `ipos < input.size()`: use `equal_range` over the sorted label range
>   `[label+i, label+first_arc[n+1])` with key `Label(input[ipos])` and the
>   `label_less` comparator (compares upper_char) to find the contiguous set of
>   arcs whose upper character equals `input[ipos]`. Let `to` be the offset of
>   `range.second` from `label`. For each arc `i` from the offset of
>   `range.first` up to `to`: push `i` onto `ca`, recurse with
>   `analyze(target_node[i], input, ipos+1, ca, analyses)` (consuming one input
>   symbol), then pop `i`.
> - No return value; results accumulate in `ca` (restored to its entry state)
>   and `analyses`.

> [spec:hfst:def:compact.sfst.compact-transducer.analyze-string-fn]
> void CompactTransducer::analyze_string( char *s, vector<CAnalysis> &analyses )

> [spec:hfst:sem:compact.sfst.compact-transducer.analyze-string-fn]
> Top-level entry point that analyzes the C-string `s` and returns all analyses
> in `analyses`.
> - Declare a local `vector<Character> input` and call
>   `alphabet.string2symseq(s, input)` to tokenize the string into a sequence of
>   symbol codes.
> - Clear `analyses`.
> - Create an empty `CAnalysis ca` and call `analyze(0, input, 0, ca, analyses)`
>   to start the recursive analysis at node 0, input position 0.
> - If `analyses.size() > 10000`, print a warning to stderr:
>   `Warning: Only the first 10000 analyses considered for "<s>"!` (with a
>   newline).
> - If `simplest_only` is true and there is more than one analysis, call
>   `disambiguate(analyses)` to keep only the simplest analyses.
> - No return value.

> [spec:hfst:def:compact.sfst.compact-transducer.arc-count-fn]
> size_t arc_count()

> [spec:hfst:sem:compact.sfst.compact-transducer.arc-count-fn]
> Inline accessor returning the `number_of_arcs` member as a `size_t`. No side
> effects.

> [spec:hfst:def:compact.sfst.compact-transducer.compact-transducer-fn]
> CompactTransducer::CompactTransducer( FILE *file, FILE *pfile )

> [spec:hfst:sem:compact.sfst.compact-transducer.compact-transducer-fn]
> Constructor that reads a compact (and optionally stochastic) transducer from
> the open files `file` and `pfile`.
> - Initialize `both_layers = false` and `simplest_only = false`.
> - Read one byte via `fgetc(file)`; if it is not `'c'`, throw the C-string
>   `"Error: wrong file format (not a compact transducer)\n"`.
> - Call `alphabet.read(file)` to load the alphabet.
> - Read `number_of_nodes` with `read_num(&number_of_nodes,
>   sizeof(number_of_nodes), file)` and `number_of_arcs` likewise.
> - If `ferror(file)` is false (no read error): allocate `finalp = new
>   char[number_of_nodes]`, `first_arc = new unsigned[number_of_nodes+1]`,
>   `label = new Label[number_of_arcs]`, `target_node = new
>   unsigned[number_of_arcs]`; then call, in order, `read_finalp(file)`,
>   `read_first_arcs(file)`, `read_labels(file)`, `read_target_nodes(file)`.
> - If `pfile == NULL`, set `arc_logprob` and `final_logprob` both to NULL;
>   otherwise call `read_probs(pfile)` to load the probability tables.

> [spec:hfst:def:compact.sfst.compact-transducer.compute-probs-fn]
> void CompactTransducer::compute_probs( vector<CAnalysis> &analyses,

> [spec:hfst:sem:compact.sfst.compact-transducer.compute-probs-fn]
> Computes a normalized probability for each analysis and sorts the analyses by
> descending probability.
> - Resize `prob` to `analyses.size()`. Initialize `sum = 0.0`.
> - For each analysis `a` at index `i`: compute `logprob = 0.0`, then for each
>   arc index `a[k]` add `arc_logprob[a[k]]`; then add
>   `final_logprob[target_node[a.back()]]` (the final-state log-prob of the
>   destination node of the last arc). Set `prob[i] = exp(logprob)` and add it
>   into `sum`.
> - Make copies `oldanalyses = analyses` and `oldprob = prob`. Then perform a
>   selection sort: for each output slot `i`, set `prob[i] = -1.0` and `n = 0`;
>   scan all `k` in `oldanalyses`, and whenever `prob[i] < oldprob[k]` set
>   `prob[i] = oldprob[k]` and `n = k` (selects the max remaining). Then set
>   `analyses[i] = oldanalyses[n]`, mark that slot consumed via
>   `oldprob[n] = -1.0`, and normalize `prob[i] /= sum`.
> - On return, `analyses` is reordered most-probable-first and `prob[i]` holds
>   the corresponding normalized probability. Note: the final stored `prob[i]`
>   is the max found divided by `sum`; because the consumed slot is set to -1.0,
>   duplicates with negative-or-equal probs are handled by the scan. No return
>   value.

> [spec:hfst:def:compact.sfst.compact-transducer.compute-score-fn]
> int compute_score( CAnalysis &ana )

> [spec:hfst:sem:compact.sfst.compact-transducer.compute-score-fn]
> Member function `int CompactTransducer::compute_score(CAnalysis &ana)` is only
> declared in compact.h; it has no definition in compact.cc or any other
> back-end source file, so there is no implemented body to port. (Disambiguation
> scoring is instead performed in `disambiguate` by converting the CAnalysis to
> an `Analysis` and calling `alphabet.compute_score`.) No behavior to
> re-implement for this symbol.

> [spec:hfst:def:compact.sfst.compact-transducer.convert-fn]
> void CompactTransducer::convert( CAnalysis &cana, Analysis &ana )

> [spec:hfst:sem:compact.sfst.compact-transducer.convert-fn]
> Converts a `CAnalysis` (`cana`, a vector of arc indices) into an `Analysis`
> (`ana`, a vector of Labels).
> - Resize `ana` to `cana.size()`.
> - For each index `i` in `0 .. cana.size()-1`, set `ana[i] = label[cana[i]]`,
>   i.e. look up the Label of the arc whose index is `cana[i]`.
> - No return value; `ana` is overwritten.

> [spec:hfst:def:compact.sfst.compact-transducer.disambiguate-fn]
> void CompactTransducer::disambiguate( vector<CAnalysis> &analyses )

> [spec:hfst:sem:compact.sfst.compact-transducer.disambiguate-fn]
> Filters `analyses` in place, keeping only the highest-scoring analyses.
> - Initialize `bestscore = INT_MIN`, an empty `vector<int> score`, and a scratch
>   `Analysis ana`.
> - For each analysis at index `i`: call `convert(analyses[i], ana)` to turn it
>   into a Label sequence, push `alphabet.compute_score(ana)` onto `score`, and
>   if `bestscore < score[i]` update `bestscore = score[i]`.
> - Compact in place: with write index `k=0`, for each `i`, if
>   `score[i] == bestscore` then assign `analyses[k++] = analyses[i]`.
> - Finally `analyses.resize(k)` so only the best-scoring analyses remain (in
>   their original relative order).
> - No return value.

> [spec:hfst:def:compact.sfst.compact-transducer.estimate-probs-fn]
> void CompactTransducer::estimate_probs( vector<double> &arcfreq,

> [spec:hfst:sem:compact.sfst.compact-transducer.estimate-probs-fn]
> Converts accumulated frequencies into normalized probabilities, mutating
> `arcfreq` and `finalfreq` in place.
> - For each node `n` in `0 .. finalfreq.size()-1`: compute `sum = finalfreq[n]`,
>   then add `arcfreq[a]` for every outgoing arc index `a` in
>   `first_arc[n] .. first_arc[n+1]-1`. If `sum == 0.0`, set `sum = 1.0` to avoid
>   division by zero.
> - Set `finalfreq[n] = finalfreq[n] / sum`, and for each outgoing arc `a` set
>   `arcfreq[a] = arcfreq[a] / sum`.
> - After all nodes, each node's final-frequency plus its outgoing-arc
>   frequencies form a normalized distribution. No return value.

> [spec:hfst:def:compact.sfst.compact-transducer.longest-match-fn]
> const char *CompactTransducer::longest_match( char* &string )

> [spec:hfst:sem:compact.sfst.compact-transducer.longest-match-fn]
> Finds the longest matching analysis starting at the current position of the
> input pointer `string` (passed by reference) and advances it past the matched
> text.
> - Declare a local `vector<char> analysis` (unused), and empty CAnalyses `ca`
>   (current) and `ba` (best). Set `l = 0`.
> - Call `longest_match2(0, string, 0, ca, l, ba)`; on return `ba` is the best
>   (longest) analysis and `l` is its matched length in input characters.
> - If `ba.size() == 0` (no match): read the next character code via
>   `alphabet.next_code(string, false, false)` (this advances `string` past that
>   character) and return `alphabet.code2symbol((Character)c)`.
> - Otherwise advance `string += l` and return `print_analysis(ba)`.
> - Returns a `const char *` symbol/analysis string; `string` is advanced as a
>   side effect.

> [spec:hfst:def:compact.sfst.compact-transducer.longest-match2-fn]
> void CompactTransducer::longest_match2(unsigned int n, char *string, int l,

> [spec:hfst:sem:compact.sfst.compact-transducer.longest-match2-fn]
> Recursive helper that searches from transducer state `n` over the remaining
> C-string `string`, tracking the current matched length `l`, the current
> analysis `ca`, the best matched length `bl` (by reference), and the best
> analysis `ba` (by reference).
> - If `finalp[n]` is non-zero and `l > bl`: record a new best, `bl = l` and
>   `ba = ca` (copy the arc vector).
> - Follow epsilon transitions: for arc `i` from `first_arc[n]` while
>   `i < first_arc[n+1]` and `label[i].upper_char() == Label::epsilon`: push `i`
>   onto `ca`, recurse `longest_match2(target_node[i], string, l, ca, bl, ba)`
>   (same `string` and `l`, since epsilon consumes no input), then pop `i`.
>   Variable `i` ends at the first non-epsilon arc.
> - Read the next input character: `end = string`,
>   `c = alphabet.next_code(end, false, false)` (advances `end` past the
>   character), and add the number of bytes consumed to `l` via
>   `l += (int)(end - string)`.
> - If `c != EOF`: use `equal_range` over `[label+i, label+first_arc[n+1])` with
>   key `Label((Character)c)` and the `label_less` comparator to find arcs whose
>   upper character equals `c`. Let `to` be the offset of `range.second`. For
>   each matching arc `i` from the offset of `range.first` up to `to`: push `i`
>   onto `ca`, recurse `longest_match2(target_node[i], end, l, ca, bl, ba)`
>   (advancing past the consumed character), then pop `i`.
> - No return value; results propagate through `bl` and `ba`. Note `l` is updated
>   before the EOF check, so the byte count of the scanned character is included
>   in `l` for the recursive calls.

> [spec:hfst:def:compact.sfst.compact-transducer.node-count-fn]
> size_t node_count()

> [spec:hfst:sem:compact.sfst.compact-transducer.node-count-fn]
> Inline accessor returning the `number_of_nodes` member as a `size_t`. No side
> effects.

> [spec:hfst:def:compact.sfst.compact-transducer.print-analysis-fn]
> char *CompactTransducer::print_analysis( CAnalysis &cana )

> [spec:hfst:sem:compact.sfst.compact-transducer.print-analysis-fn]
> Renders a `CAnalysis` `cana` into a printable analysis string.
> - Declare a local `Analysis ana`, call `convert(cana, ana)` to turn arc indices
>   into a Label sequence.
> - Return `alphabet.print_analysis(ana, both_layers)`, passing the `both_layers`
>   member to control whether both surface and analysis layers are printed.
> - Returns the `char *` produced by `Alphabet::print_analysis`.

> [spec:hfst:def:compact.sfst.compact-transducer.read-finalp-fn]
> void CompactTransducer::read_finalp( FILE *file )

> [spec:hfst:sem:compact.sfst.compact-transducer.read-finalp-fn]
> Reads the final-state bit vector from `file` into the `finalp` array, one bit
> per node, packed 8 bits per byte, most-significant bit first.
> - Initialize `k = 0` (remaining bits in current byte) and `n = 0` (current
>   byte, `unsigned char`).
> - For each node `i` in `0 .. number_of_nodes-1`: if `k == 0`, read a fresh byte
>   `n = (unsigned char)fgetc(file)` and set `k = 8`. Decrement `k`. Then if
>   `n & (1 << k)` is non-zero set `finalp[i] = 1`, else `finalp[i] = 0`.
> - Bits are consumed from the most significant (bit 7) downward within each
>   byte. No return value.

> [spec:hfst:def:compact.sfst.compact-transducer.read-first-arcs-fn]
> void CompactTransducer::read_first_arcs( FILE *file )

> [spec:hfst:sem:compact.sfst.compact-transducer.read-first-arcs-fn]
> Reads the `first_arc` array (`number_of_nodes+1` entries) from `file` as a
> packed bit stream, each value `bits` wide where
> `bits = (int)ceil(log(number_of_arcs+1)/log(2))` (bits per value, base-2 log).
> - Maintain `k = 0` (number of valid high bits currently buffered in `n`) and a
>   32-bit unsigned accumulator `n = 0`. `sizeof(n)*8` is the accumulator width
>   in bits.
> - For each `i` in `0 .. number_of_nodes`:
>   - Set `first_arc[i] = n >> (sizeof(n)*8 - bits)` (take the top `bits` bits of
>     `n`).
>   - Shift `n <<= bits` and `k -= bits`.
>   - If `k < 0` (the buffer underflowed): read a fresh word via
>     `read_num(&n, sizeof(n), file)`, OR in the missing low bits with
>     `first_arc[i] |= n >> (sizeof(n)*8 + k)` (note `k` is negative here), then
>     consume those used bits with `n <<= -k` and replenish
>     `k += (int)sizeof(n)*8`.
> - Effectively this MSB-first bit reader produces `number_of_nodes+1` values of
>   `bits` bits each. No return value.

> [spec:hfst:def:compact.sfst.compact-transducer.read-labels-fn]
> void CompactTransducer::read_labels( FILE *file )

> [spec:hfst:sem:compact.sfst.compact-transducer.read-labels-fn]
> Reads the `label` array (`number_of_arcs` entries) from `file` as a packed bit
> stream of alphabet indices, then maps each index to a Label.
> - Build a lookup table `Num2Label` (HFST allocates it dynamically with
>   `new Label[alphabet.size()]`): iterate the alphabet via its `const_iterator`
>   from `begin()` to `end()`, assigning `Num2Label[N++] = *it`, so entry `j`
>   holds the j-th alphabet Label in iteration order.
> - Set `bits = (int)ceil(log((double)alphabet.size())/log(2))` (bits per index).
> - Use the same MSB-first packed bit reader as `read_first_arcs`/
>   `read_target_nodes` with accumulator `n` and bit count `k` (both start 0):
>   for each arc `i` in `0 .. number_of_arcs-1`, decode an index `l`: set
>   `l = n >> (sizeof(n)*8 - bits)`, `n <<= bits`, `k -= bits`; if `k < 0` read a
>   fresh word via `read_num(&n, sizeof(n), file)`, OR in
>   `l |= n >> (sizeof(n)*8 + k)`, then `n <<= -k` and `k += sizeof(n)*8`.
> - Set `label[i] = Num2Label[l]`.
> - After the loop, free `Num2Label` with `delete[]` (HFST addition). No return
>   value.

> [spec:hfst:def:compact.sfst.compact-transducer.read-probs-fn]
> void CompactTransducer::read_probs( FILE *file )

> [spec:hfst:sem:compact.sfst.compact-transducer.read-probs-fn]
> Reads the final-state and arc log-probability tables from `file` (the
> probability file) into `final_logprob` and `arc_logprob`.
> - Read `n` (size_t) via `fread(&n, sizeof(n), 1, file)`; if it does not read 1
>   item, throw the C-string `"read_probs: fread failed"` (HFST addition).
> - Read `m` (size_t) via `fread(&m, sizeof(n), 1, file)`. If that read fails, or
>   `n != node_count()`, or `m != arc_count()`, print
>   `Error: incompatible probability file!` to stderr and call `exit(1)`.
> - Allocate `final_logprob = new float[n]` and `arc_logprob = new float[m]`.
> - Read `n` floats into `final_logprob` via `fread(...)`; if fewer than `n` are
>   read, throw `"read_probs: fread failed"`.
> - Read into `arc_logprob` via `fread(arc_logprob, sizeof(float), n, file)`
>   (note: the count passed is `n`, the node count, not `m`); if the result is
>   not `n`, print `Error: in probability file!` to stderr and call `exit(1)`.
> - No return value. (The arc read uses `n` as the count, matching the source
>   exactly.)

> [spec:hfst:def:compact.sfst.compact-transducer.read-target-nodes-fn]
> void CompactTransducer::read_target_nodes( FILE *file )

> [spec:hfst:sem:compact.sfst.compact-transducer.read-target-nodes-fn]
> Reads the `target_node` array (`number_of_arcs` entries) from `file` as a
> packed bit stream, each value `bits` wide where
> `bits = (int)ceil(log(number_of_nodes)/log(2))`.
> - Same MSB-first packed bit reader as `read_first_arcs`: accumulator `n` and
>   bit count `k` both start 0. For each arc `i` in `0 .. number_of_arcs-1`:
>   set `target_node[i] = n >> (sizeof(n)*8 - bits)`, then `n <<= bits` and
>   `k -= bits`; if `k < 0`, read a fresh word via
>   `read_num(&n, sizeof(n), file)`, OR in `target_node[i] |= n >>
>   (sizeof(n)*8 + k)`, then `n <<= -k` and `k += (int)sizeof(n)*8`.
> - No return value.

> [spec:hfst:def:compact.sfst.compact-transducer.robust-analyze-string-fn]
> float robust_analyze_string( char *string, std::vector<CAnalysis> &analyses,

> [spec:hfst:sem:compact.sfst.compact-transducer.robust-analyze-string-fn]
> Member function `float CompactTransducer::robust_analyze_string(char *string,
> std::vector<CAnalysis> &analyses, float ErrorsAllowed)` is only declared in
> compact.h; it has no definition in compact.cc or any other back-end source
> file, so there is no implemented body to port. No behavior to re-implement for
> this symbol.

> [spec:hfst:def:compact.sfst.compact-transducer.train-fn]
> bool CompactTransducer::train( char *s, vector<double> &arcfreq,

> [spec:hfst:sem:compact.sfst.compact-transducer.train-fn]
> EM-style training step: analyzes string `s` and distributes fractional counts
> over the arcs and final states used by its analyses into `arcfreq` and
> `finalfreq`.
> - Tokenize `s` into `vector<Character> input` via
>   `alphabet.string2symseq(s, input)`. Create empty `vector<CAnalysis> analyses`
>   and `CAnalysis ca`, then call `analyze(0, input, 0, ca, analyses)`.
> - If `analyses.size() > 10000`, return `true` (ignore over-ambiguous inputs).
>   Else if `analyses.size() == 0`, return `false` (input not covered).
> - If `simplest_only` and more than one analysis, call `disambiguate(analyses)`.
> - If there is at least one analysis: let `incr = 1.0 / analyses.size()` (the
>   fractional weight per analysis). For each analysis `arcs`: for each arc index
>   `arcs[k]`, add `incr` to `arcfreq[arcs[k]]`; and add `incr` to
>   `finalfreq[target_node[arcs.back()]]` (the final node reached by the last
>   arc).
> - Return `true`. (`arcfreq`/`finalfreq` are accumulated, not reset.)

> [spec:hfst:def:compact.sfst.compact-transducer.train2-fn]
> bool CompactTransducer::train2( char *s, vector<double> &arcfreq,

> [spec:hfst:sem:compact.sfst.compact-transducer.train2-fn]
> Training step that follows a single deterministic path matching the full
> input/output label sequence of `s`, adding integer counts to `arcfreq` and
> `finalfreq`.
> - Tokenize `s` into `vector<Label> input` via
>   `alphabet.string2labelseq(s, input)` (full labels, both layers, not just
>   upper symbols). Create empty `CAnalysis ca`, `n = 0`, `failure = false`.
> - For each input label `input[i]`: set `failure = true`, then scan the outgoing
>   arcs of node `n` (indices `first_arc[n] .. first_arc[n+1]-1`); if
>   `label[k] == input[i]`, push `k` onto `ca`, advance `n = target_node[k]`, set
>   `failure = false`, and break. If after the scan `failure` is still true,
>   break out of the input loop.
> - If `failure` is true (a label did not match) or the final node is not final
>   (`!finalp[n]`): print to stderr
>   `Warning: The following input is not covered:\n<s>\n` and return `false`.
> - Otherwise, for each arc index `ca[k]` add 1 to `arcfreq[ca[k]]`, and add 1 to
>   `finalfreq[target_node[ca.back()]]`. Return `true`.

> [spec:hfst:def:compact.sfst.label-less]
> class label_less

> [spec:hfst:def:compact.sfst.label-less.operator-fn]
> bool operator()(const Label l1, const Label l2) const

> [spec:hfst:sem:compact.sfst.label-less.operator-fn]
> Comparator `operator()(const Label l1, const Label l2)` returning
> `l1.upper_char() < l2.upper_char()`, i.e. orders Labels strictly by their upper
> (input) character. Used as the ordering for `equal_range` over the arc label
> arrays. No side effects.

