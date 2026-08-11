# libhfst/src/parsers/XfstCompiler.cc, libhfst/src/parsers/XfstCompiler.h

> [spec:hfst:def:xfst-compiler.hfst.xfst.allow-char-fn]
> static bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.allow-char-fn]
> Free static helper `allow_char(char c) -> bool`. Defines the literal set
> of "allowed" boundary characters as the string `" \n\t.,;:?!-/'\"<>()|"`
> (space, newline, tab, period, comma, semicolon, colon, question mark,
> exclamation mark, hyphen, slash, apostrophe, double-quote, angle brackets,
> parentheses, pipe). Iterates over that string; returns true if `c` equals
> any of those characters, otherwise returns false. No state, no side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.append-state-to-paths-fn]
> static void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.append-state-to-paths-fn]
> Free static helper `append_state_to_paths(whole_path, shortest_path, state)`
> used by net inspection. Both paths are vectors of unsigned int state ids,
> passed by mutable reference; `state` is the state being entered.
> Step 1: unconditionally append `state` to the end of `whole_path`.
> Step 2: scan `shortest_path` from the front; if `state` already appears in
> it, erase everything from that first occurrence to the end (truncating the
> path back to before the loop) and stop scanning. Step 3: append `state` to
> the end of `shortest_path`. Net effect: `shortest_path` becomes a loop-free
> path that ends at `state`, while `whole_path` records every state visited.
> Returns void.

> [spec:hfst:def:xfst-compiler.hfst.xfst.apply-direction]
> enum ApplyDirection {
>   APPLY_UP_DIRECTION;
>   APPLY_DOWN_DIRECTION;
> }

> [spec:hfst:def:xfst-compiler.hfst.xfst.binary-operation]
> enum BinaryOperation {
>   IGNORE_NET;
>   INTERSECT_NET;
>   COMPOSE_NET;
>   CONCATENATE_NET;
>   MINUS_NET;
>   UNION_NET;
>   SHUFFLE_NET;
>   CROSSPRODUCT_NET;
> }

> [spec:hfst:def:xfst-compiler.hfst.xfst.contains-regexp-markers-on-one-side-fn]
> static HfstTransducer *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.contains-regexp-markers-on-one-side-fn]
> Free static helper `contains_regexp_markers_on_one_side(xre_, input_side)`
> returning a newly-allocated `HfstTransducer*`. Compiles, via the passed
> XreCompiler, a regular expression that matches any path having a `^[` or
> `^]` marker on exactly one side. If `input_side` is true, compiles
> `[?:?|0:?|?:0]* ["^[":? | "^]":? | "^[":0 | "^]":0] [?:?|0:?|?:0]*`
> (the marker appears on the input/upper side). Otherwise compiles
> `[?:?|0:?|?:0]* [?:"^[" | ?:"^]" | 0:"^[" | 0:"^]"] [?:?|0:?|?:0]*`
> (marker on output/lower side). Asserts the compile result is non-NULL and
> returns it; ownership passes to the caller.

> [spec:hfst:def:xfst-compiler.hfst.xfst.contains-regexps-fn]
> static HfstTransducer *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.contains-regexps-fn]
> Free static helper `contains_regexps(xre_) -> HfstTransducer*`. Builds an
> automaton accepting every path that contains one or more well-formed
> `^[ ... ^]` regexp sub-expressions. Steps: (1) compile
> `[? - "^[" - "^]"]* ;` into `not_bracket_star` (any string of symbols other
> than the two bracket markers). (2) define it under the temporary name
> `TempNotBracketStar` in the XreCompiler. (3) compile the well-formed
> expression
> `TempNotBracketStar "^[" TempNotBracketStar [ "^]" TempNotBracketStar "^[" TempNotBracketStar ]* "^]" TempNotBracketStar ;`
> into `well_formed`. (4) undefine `TempNotBracketStar`, delete the
> `not_bracket_star` transducer. Returns `well_formed`; ownership passes to
> the caller.

> [spec:hfst:def:xfst-compiler.hfst.xfst.convert-argument-symbols-fn]
> std::string

> [spec:hfst:sem:xfst-compiler.hfst.xfst.convert-argument-symbols-fn]
> `convert_argument_symbols(arguments, xre, function_name, xre_, user_friendly_argument_names=false) -> std::string`.
> Rewrites the regex string `xre` so each occurrence of a function argument
> name is replaced by a placeholder symbol. Starts with `retval = xre` and
> `arg_number = 1`. For each argument name (in order):
> (1) call `xre_.get_positions_of_symbol_in_xre(argument, retval, arg_positions)`
> to find the set of character offsets where the argument symbol occurs in
> the current `retval`; if that call returns false, abort and return the
> empty string `""`. (2) build the substituting token: if
> `user_friendly_argument_names`, it is `"ARGUMENT" + arg_number`; otherwise
> it is `"\"@" + function_name + arg_number + "@\""` (e.g. `"@Foo1@"`).
> (3) scan `retval` char by char building `new_retval`: when index `i` is in
> `arg_positions`, erase `i` from the set, append the substituting token, and
> advance `i` past the rest of the original argument symbol (skip
> `argument.length()-1` further chars); otherwise copy the current char.
> (4) set `retval = new_retval`, increment `arg_number`. After all arguments,
> return `retval`. Note the per-argument replacement uses the offsets
> computed against the current (already partly rewritten) `retval`.

> [spec:hfst:def:xfst-compiler.hfst.xfst.copied-stack-fn]
> std::stack<HfstTransducer *> copied_stack(stack_)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.copied-stack-fn]
> Inside `test_operation`: `std::stack<HfstTransducer*> copied_stack(stack_)`
> copy-constructs a local stack from the member `stack_`. This is a shallow
> copy of the pointer stack: it duplicates the sequence of `HfstTransducer*`
> entries (same pointee transducers) so the local copy can be popped/iterated
> without mutating the compiler's real stack. The transducers themselves are
> not cloned at this point (callers later copy-construct individual
> transducers when reading `.top()`).

> [spec:hfst:def:xfst-compiler.hfst.xfst.extract-function-arguments-fn]
> static bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-function-arguments-fn]
> Free static helper `extract_function_arguments(prototype, args) -> bool`
> where `prototype` is a C string of form `"functionname(arg1, arg2, ... argN)"`
> and `args` is an output `std::vector<std::string>`. Steps: (1) advance an
> index `i` until `prototype[i] == '('`; if a NUL terminator is reached first,
> return false. (2) skip the `'('`. (3) scan from there until `')'`, building
> up the current argument string `arg`: a NUL before `')'` returns false (no
> closing parenthesis); a space is skipped; a comma ends the current argument
> — push `arg` into `args` and reset `arg` to empty; any other char is
> appended to `arg`. (4) after the loop (at `')'`), push the final `arg`.
> Return true. Note: whitespace inside argument names is stripped entirely,
> and an empty last argument is still pushed.

> [spec:hfst:def:xfst-compiler.hfst.xfst.extract-function-name-fn]
> static bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-function-name-fn]
> Free static helper `extract_function_name(prototype, name) -> bool` where
> `prototype` is a C string and `name` is an output `std::string` (appended
> to). Iterates over `prototype` until the NUL terminator: appends each
> character (including the `'('` itself) to `name`, and as soon as a `'('` is
> appended, returns true. If the string ends with no `'('` found, returns
> false. So on success `name` ends with `(` and contains the function name
> plus the opening parenthesis.

> [spec:hfst:def:xfst-compiler.hfst.xfst.extract-output-paths-fn]
> static HfstOneLevelPaths

> [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-output-paths-fn]
> Free static helper `extract_output_paths(paths) -> HfstOneLevelPaths`.
> Takes `HfstTwoLevelPaths` (a collection of (weight, StringPairVector)
> entries) and projects each path onto its output (second) side. For each
> entry: build a new `StringVector new_path` by iterating the StringPairVector;
> for each symbol pair, look at `p.second` (the output symbol) and skip it if
> it equals `"@0@"` or `"@_EPSILON_SYMBOL_@"`; if it equals
> `"@_UNKNOWN_SYMBOL_@"`, push the literal `"?"` instead; otherwise push
> `p.second` verbatim. Insert `{ it.first /*weight*/, new_path }` into the
> result map. Returns the assembled `HfstOneLevelPaths`.

> [spec:hfst:def:xfst-compiler.hfst.xfst.initialize-variable-explanations-fn]
> static void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.initialize-variable-explanations-fn]
> Free static `initialize_variable_explanations()` returning void. Populates
> the file-static map `variable_explanations_` (string variable name ->
> human-readable explanation string) with a fixed set of entries, one
> assignment per known xfst variable. The entries (key = explanation): assert
> = "quit the application if test result is 0 and quit-on-fail is ON";
> att-epsilon = "epsilon symbol used when reading from att files";
> char-encoding = "character encoding used"; copyright-owner = "";
> directory = "<NOT IMPLEMENTED>"; encode-weights = "encode weights when
> minimizing"; flag-is-epsilon = "treat flag diacritics as epsilons in
> composition"; harmonize-flags = "harmonize flag diacritics before
> composition"; hopcroft-min = "use hopcroft's minimization algorithm";
> lexc-minimize-flags, lexc-rename-flags, lexc-with-flags (lexc flag handling
> descriptions); maximum-weight = "maximum weight of paths printed in apply";
> minimal = "minimize networks after operations"; name-nets = "stores the
> name of the network when using 'define'"; obey-flags = "obey flag diacritic
> constraints"; precision = "todo: precision to use when printing weights";
> print-foma-sigma = "print identities as '@'"; print-pairs, print-sigma,
> print-space, print-weight (print-related descriptions); quit-on-fail =
> "quit the application if a command cannot be executed"; quote-special =
> "enclose special characters in double quotes"; random-seed =
> "<EXPLANATION MISSING>"; recode-cp1252 = "<NOT SUPPORTED>";
> recursive-define = "<EXPLANATION MISSING>"; retokenize = "retokenize
> regular expressions in 'compile-replace'"; show-flags = "show flag
> diacritics when printing"; sort-arcs = "<NOT IMPLEMENTED>"; use-timer =
> "<NOT IMPLEMENTED>"; verbose = "print more information"; xerox-composition
> = "treat flag diacritics as ordinary symbols in composition". This is a
> one-time static-table initializer with no parameters and no return value.

> [spec:hfst:def:xfst-compiler.hfst.xfst.intersection-fn]
> HfstTransducer intersection(topmost_transducer)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.intersection-fn]
> Inside `test_operation`'s TEST_SUBLANGUAGE_ case:
> `HfstTransducer intersection(topmost_transducer)` copy-constructs a working
> transducer from the current `topmost_transducer`, then
> `intersection.intersect(next_transducer)` mutates it in place to the
> intersection of the two. It is used to test sublanguage: if
> `intersection.compare(topmost_transducer)` is false (i.e. the intersection
> does not equal `topmost_transducer`, meaning `topmost_transducer` is not a
> sublanguage of `next_transducer`), the test prints false and returns;
> otherwise `topmost_transducer` is reassigned to `next_transducer` and the
> loop continues. `intersection` is a temporary local copy and does not alter
> the stack.

> [spec:hfst:def:xfst-compiler.hfst.xfst.is-special-symbol-fn]
> static bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.is-special-symbol-fn]
> Free static helper `is_special_symbol(const std::string& s) -> bool`.
> Returns true if `s` equals any of the three HFST internal special-symbol
> constants `hfst::internal_epsilon`, `hfst::internal_unknown`, or
> `hfst::internal_identity`; otherwise returns false. Pure predicate, no
> side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.is-unknown-or-identity-used-in-transducer-fn]
> static bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.is-unknown-or-identity-used-in-transducer-fn]
> Free static helper
> `is_unknown_or_identity_used_in_transducer(t, unknown, identity) -> bool`.
> `unknown` and `identity` are bool output references. Sets both to false,
> then builds an `HfstBasicTransducer fsm(*t)` and iterates over every state
> and every transition. For each transition it reads `istr` and `ostr` — NOTE
> the C++ assigns BOTH from `get_input_symbol()` (so the output side is never
> actually examined; this is the literal behavior to preserve). If `istr` or
> `ostr` equals `hfst::internal_unknown`, set `unknown = true`; else if it
> equals `hfst::internal_identity`, set `identity = true`. As soon as both
> `unknown` and `identity` are true, return true early. After the loops,
> return true if either flag is set, otherwise false.

> [spec:hfst:def:xfst-compiler.hfst.xfst.is-valid-string-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.is-valid-string-fn]
> `is_valid_string(const StringVector& sv) -> bool`. Evaluates a flag-diacritic
> sequence to decide whether the string satisfies all flag constraints. Keeps
> two structures: `values` (map feature -> current value string) and
> `negative_values` (set of features whose value was negatively set). Iterates
> the symbols in `sv`; non-diacritic symbols are ignored. For each diacritic
> (tested via `FdOperation::is_diacritic`), extract single-char operator `op`
> (assert operator string length is 1), feature `feat`, value `val`, and
> `is_negatively_set = (feat in negative_values)`. Then switch on `op`:
> 'P' (positive set): `values[feat] = val`. 'N' (negative set):
> `values[feat] = val` and add `feat` to `negative_values`. 'R' (require):
> only checked when `val` is empty in the code — if `val.empty()`: when
> `values[feat]` is empty return false, otherwise (nonempty branch) if
> negatively set or `values[feat] != val` return false. 'D' (disallow): if
> `val` empty, return false when `values[feat] != ""`; if `val` nonempty,
> return false when not negatively set and `values[feat] == val`.
> 'C' (clear): `values[feat] = ""`. 'U' (unification): if the feature is
> unset, or (not negatively set and `values[feat] == val`), or (negatively
> set and `values[feat] != val`), then set `values[feat] = val`; otherwise
> return false. default: print an error line to std::cerr and `throw;`
> (rethrow). If the whole sequence is processed without returning false,
> return true.

> [spec:hfst:def:xfst-compiler.hfst.xfst.is-well-formed-for-compile-replace-fn]
> static bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.is-well-formed-for-compile-replace-fn]
> Free static helper `is_well_formed_for_compile_replace(t, xre_) -> bool`,
> precondition: `t` is an automaton (acceptor). Steps: (1) `well_formed =
> contains_regexps(xre_)` (all paths containing one or more well-formed
> `^[ ... ^]` expressions). (2) copy `t` into `tc` and `tc.subtract(*well_formed)`
> to remove the well-formed paths; delete `well_formed`. (3) compile
> `brackets = xre_.compile("$[ \"^[\" | \"^]\" ] ;")` (all paths containing
> at least one `^[` or `^]`). (4) `tc.intersect(*brackets)`; delete `brackets`.
> (5) construct an `empty` transducer of `tc`'s type, and return
> `empty.compare(tc, false)` — i.e. return true iff `tc` is empty after
> subtracting well-formed regexps and intersecting with bracket-containing
> paths (meaning no malformed/leftover bracket paths remain). The `false`
> argument to compare disables weight comparison.

> [spec:hfst:def:xfst-compiler.hfst.xfst.labelpair-fn]
> StringPair labelpair(labelstr, labelstr)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.labelpair-fn]
> Inside `substitute_named`: `StringPair labelpair(labelstr, labelstr)`
> constructs a symbol pair whose input and output sides are both the already
> normalized `labelstr` (the substitution target label, with `?`/`0` mapped to
> the internal identity/epsilon symbols). It is then used in
> `top->substitute(labelpair, *(it->second), false)` to replace every
> matching label pair on the top transducer with the definition transducer.

> [spec:hfst:def:xfst-compiler.hfst.xfst.labelstr-fn]
> std::string labelstr(label)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.labelstr-fn]
> Inside `substitute_named`: `std::string labelstr(label)` copies the C-string
> `label` argument into a std::string, then normalizes it: if it equals `"?"`
> it is replaced with `"@_IDENTITY_SYMBOL_@"`; if it equals `"0"` it is
> replaced with `"@_EPSILON_SYMBOL_@"`. The normalized `labelstr` is then used
> to look the symbol up in the top transducer's alphabet (erroring if absent)
> and as the substitution target.

> [spec:hfst:def:xfst-compiler.hfst.xfst.level]
> enum Level {
>   LOWER_LEVEL;
>   UPPER_LEVEL;
>   BOTH_LEVELS;
> }

> [spec:hfst:def:xfst-compiler.hfst.xfst.line-tr-fn]
> HfstTransducer line_tr(spv, format_)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.line-tr-fn]
> Inside the read-text/read-spaced file loop: `HfstTransducer line_tr(spv, format_)`
> constructs a transducer of implementation type `format_` from `spv`, the
> `StringPairVector` produced by tokenizing the current input line into
> symbol pairs. This `line_tr` represents the single string/path of that
> line; it is then disjuncted into the accumulator transducer
> (`tmp->disjunct(line_tr)`), building up the union of all lines.

> [spec:hfst:def:xfst-compiler.hfst.xfst.liststr-fn]
> std::string liststr(list)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.liststr-fn]
> Inside `substitute_symbol`: `std::string liststr(list)` copies the C-string
> `list` (the replacement symbol-list expression) into a std::string. Special
> case: if `liststr == "\"NOTHING\""` it is set to the empty string (meaning
> substitute the target with nothing/epsilon). The value is then spliced into
> a constructed regex `` `[ [TempXfstTransducerName] , "<target>" , <liststr> ] ``
> that is compiled to perform the substitution.

> [spec:hfst:def:xfst-compiler.hfst.xfst.name-fn]
> std::string name_(name)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.name-fn]
> Inside `eliminate_flag`: `std::string name_(name)` copies the C-string flag
> `name` argument into a local std::string `name_`. (Note: the actual
> elimination call below uses the raw `name` pointer, so `name_` is just a
> local copy of the flag name kept in scope.)

> [spec:hfst:def:xfst-compiler.hfst.xfst.return-to-level-fn]
> static bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.return-to-level-fn]
> Free static helper `return_to_level(whole_path, shortest_path, level) -> bool`,
> used by `inspect_net` to step back up to depth `level`. If
> `whole_path.size() < level` or `level == 0`, return false (invalid).
> Otherwise: erase `whole_path` from index `level` to the end (so it keeps the
> first `level` entries). Take `state = whole_path.back()` (the new last
> state). Then scan `shortest_path` from the front; if `state` appears, erase
> from that occurrence to the end and stop. Append `state` to `shortest_path`.
> Return true. Mirrors append_state_to_paths but truncates `whole_path` to a
> given level rather than appending one state.

> [spec:hfst:def:xfst-compiler.hfst.xfst.string-found-fn]
> static bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.string-found-fn]
> Free static helper `string_found(str_, text_) -> bool`. Decides whether the
> word `str_` occurs as a whole word inside `text_`, case-insensitively and
> respecting punctuation boundaries. Steps: uppercase both via `to_upper_case`
> giving `str` and `text`; find the first occurrence `pos = text.find(str)`;
> if `pos == npos` return false. Then check word boundaries: the left side is
> OK if `pos == 0` or the preceding char `text[pos-1]` is an allowed boundary
> char (`allow_char`); if left is OK, the right side is OK if
> `pos + str.length() == text.length()` (end of text) or the following char
> `text[pos + str.length()]` is an allowed boundary char. Return true only if
> both boundaries are satisfied, else false. (Note: only the first occurrence
> is checked.) This function is inside an `#if`-guarded block.

> [spec:hfst:def:xfst-compiler.hfst.xfst.string-map]
> typedef std::map<std::string,std::string> StringMap

> [spec:hfst:def:xfst-compiler.hfst.xfst.string-to-float-fn]
> static float

> [spec:hfst:sem:xfst-compiler.hfst.xfst.string-to-float-fn]
> Free static helper `string_to_float(const std::string& str) -> float`.
> Constructs an `istringstream` over `str` and extracts a single float via
> `iss >> f`, returning `f`. Standard stream parsing: leading whitespace is
> skipped, parsing stops at the first non-numeric char; if extraction fails,
> `f` is left default-initialized (0). No error reporting.

> [spec:hfst:def:xfst-compiler.hfst.xfst.string-to-size-t-fn]
> static size_t

> [spec:hfst:sem:xfst-compiler.hfst.xfst.string-to-size-t-fn]
> Free static helper `string_to_size_t(const std::string& str) -> size_t`.
> Constructs an `istringstream` over `str` and extracts a single `size_t` via
> `iss >> size`, returning it. Standard stream parsing semantics (skip leading
> whitespace, stop at first non-digit; on failure the value is
> default/zero). No error reporting.

> [spec:hfst:def:xfst-compiler.hfst.xfst.symbol-vector-to-symbol-pair-fn]
> static StringPair

> [spec:hfst:sem:xfst-compiler.hfst.xfst.symbol-vector-to-symbol-pair-fn]
> Free static helper `symbol_vector_to_symbol_pair(const StringVector& sv) -> StringPair`.
> Converts a 1- or 2-element symbol vector into an input:output symbol pair,
> mapping the xfst literals `?` and `0` to internal symbols.
> If `sv.size() == 2`: set `sp.first` from `sv[0]` (`?` -> "@_UNKNOWN_SYMBOL_@",
> `0` -> "@_EPSILON_SYMBOL_@", else verbatim), and `sp.second` from `sv[1]`
> using the same mapping. If `sv.size() == 1`: set `sp.first` from `sv[0]`
> where `?` maps to "@_IDENTITY_SYMBOL_@" (NOTE: identity, not unknown, in the
> single-symbol case), `0` maps to "@_EPSILON_SYMBOL_@", else verbatim; then
> `sp.second = sp.first` (identity pair). Any other size: `throw` a C-string
> "error: symbol vector cannot be converted into symbol pair". Return `sp`.

> [spec:hfst:def:xfst-compiler.hfst.xfst.temp-fn]
> HfstTransducer temp(format_)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.temp-fn]
> Inside `print_words(name, number, oss_, level)`: `HfstTransducer temp(format_)`
> constructs an empty working transducer of implementation type `format_`. If
> `name == NULL`, it is assigned a copy of the current stack top
> (`this->top()`; returns `*this` early if the stack is empty); otherwise it is
> assigned a copy of the named definition looked up in `definitions_`. `temp`
> then holds the transducer whose words are enumerated.

> [spec:hfst:def:xfst-compiler.hfst.xfst.test-operation]
> enum TestOperation {
>   TEST_SUBLANGUAGE_;
>   TEST_OVERLAP_;
> }

> [spec:hfst:def:xfst-compiler.hfst.xfst.tmp-fn]
> HfstTransducer tmp(format_)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.tmp-fn]
> Inside `print_random_lower`: `HfstTransducer tmp(format_)` constructs an
> empty working transducer of implementation type `format_`. If `name == NULL`,
> it is assigned a copy of the current stack top (`this->top()`; returns
> `*this` early if the stack is empty); otherwise it is assigned a copy of the
> named definition from `definitions_` (erroring if not found). `tmp` is the
> source transducer from which random lower-side paths are extracted.

> [spec:hfst:def:xfst-compiler.hfst.xfst.to-filename-fn]
> static const char *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.to-filename-fn]
> Free static helper `to_filename(const char* file) -> const char*`. If `file`
> is null (0), return the literal string `"<stdin>"`; otherwise return `file`
> unchanged. Used to produce a display name for a possibly-null filename.

> [spec:hfst:def:xfst-compiler.hfst.xfst.to-literal-regexp-fn]
> static std::string

> [spec:hfst:sem:xfst-compiler.hfst.xfst.to-literal-regexp-fn]
> Free static helper `to_literal_regexp(path, input_side) -> std::string`.
> Builds a regex string accepting exactly the literal symbols of one side of a
> path. Start `pathstr = "["`. For each symbol pair in `path`, pick `symbol =
> input_side ? it.first : it.second`; if `symbol != hfst::internal_epsilon`,
> append `"<symbol>" ` (the symbol wrapped in double quotes followed by a
> space). Append `"]"`. If the result is exactly `"[]"` (path was all
> epsilon/empty), replace it with `"[0]"`. Return `pathstr`.

> [spec:hfst:def:xfst-compiler.hfst.xfst.to-regexp-fn]
> static std::string

> [spec:hfst:sem:xfst-compiler.hfst.xfst.to-regexp-fn]
> Free static helper `to_regexp(path, input_side, retokenize) -> std::string`.
> Builds a regex string from one side of a path, but unlike to_literal_regexp
> the symbols are emitted unquoted (so they are reparsed as regex) and bracket
> markers are turned into a placeholder. Start `pathstr = "["`. For each
> symbol pair, pick `symbol = input_side ? it.first : it.second`. If `symbol`
> is neither `"^]"` nor `"^["`: if it is not `hfst::internal_epsilon`, append
> the symbol (and, unless `retokenize`, append a trailing space); epsilons are
> skipped. If `symbol` IS `"^["` or `"^]"`: append the literal
> `"@EPSILON_MARKER@"` (quoted) and, unless `retokenize`, a trailing space.
> Append `"]"`. If the result equals `"[]"`, replace with `"[0]"`. Return
> `pathstr`. When `retokenize` is true, spaces are omitted so the symbols can
> be re-tokenized as a continuous regex.

> [spec:hfst:def:xfst-compiler.hfst.xfst.to-upper-case-fn]
> static std::string

> [spec:hfst:sem:xfst-compiler.hfst.xfst.to-upper-case-fn]
> Free static helper `to_upper_case(const std::string& str) -> std::string`.
> Builds `retval` by iterating each byte of `str`: if the byte is in the ASCII
> lowercase range (>= 97 'a' and <= 122 'z'), append the byte minus 32 (its
> uppercase ASCII equivalent); otherwise append the byte unchanged. Operates on
> raw bytes (ASCII only), so non-ASCII/multibyte chars pass through verbatim.
> Returns the assembled string.

> [spec:hfst:def:xfst-compiler.hfst.xfst.tok-fn]
> HfstStrings2FstTokenizer tok(mcs, hfst::internal_epsilon)

> [spec:hfst:sem:xfst-compiler.hfst.xfst.tok-fn]
> Inside `read_text_or_spaced` (the read-text/read-spaced file loop):
> `HfstStrings2FstTokenizer tok(mcs, hfst::internal_epsilon)` constructs a
> string-to-fst tokenizer. `mcs` is an empty `StringVector` (no multichar
> symbols are registered), and `hfst::internal_epsilon` ("@_EPSILON_SYMBOL_@")
> is passed as the epsilon symbol the tokenizer should recognize. The tokenizer
> is then reused for each input line via `tok.tokenize_pair_string(line, spaces)`
> to split the line into a `StringPairVector`.

> [spec:hfst:def:xfst-compiler.hfst.xfst.tokenize-string-fn]
> static StringVector

> [spec:hfst:sem:xfst-compiler.hfst.xfst.tokenize-string-fn]
> Free static helper `tokenize_string(const char* s, char c) -> StringVector`.
> Splits the C-string `s` into substrings on every occurrence of the delimiter
> char `c`. Copies `s` into `std::string str`; tracks a segment start `pos = 0`.
> Iterates index `i` over `str`: when `str[i] == c`, push the substring
> `str[pos .. i)` (length `i - pos`) and set `pos = i + 1`. After the loop push
> the final trailing substring `str[pos .. end)`. Always returns at least one
> element; consecutive or trailing delimiters yield empty-string segments.

> [spec:hfst:def:xfst-compiler.hfst.xfst.unary-operation]
> enum UnaryOperation {
>   DETERMINIZE_NET;
>   EPSILON_REMOVE_NET;
>   INVERT_NET;
>   LOWER_SIDE_NET;
>   UPPER_SIDE_NET;
>   OPTIONAL_NET;
>   ONE_PLUS_NET;
>   ZERO_PLUS_NET;
>   REVERSE_NET;
>   MINIMIZE_NET;
>   PRUNE_NET_;
> }

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler]
> class XfstCompiler {
>   XfstCompiler& add_props(FILE* infile);
>   XfstCompiler& add_props(const char* indata);
>   XfstCompiler& apply_up(FILE* infile);
>   XfstCompiler& apply_up(const char* indata);
>   XfstCompiler& apply_down(FILE* infile);
>   XfstCompiler& apply_down(const char* indata);
>   XfstCompiler& apply_med(FILE* infile);
>   XfstCompiler& apply_med(const char* indata);
>   XfstCompiler& lookup_optimize();
>   XfstCompiler& remove_optimization();
>   XfstCompiler& define_alias(const char* name, const char* commands);
>   XfstCompiler& define_list(const char* name, const char* start, const char* end);
>   XfstCompiler& define_list(const char* name, const char* list);
>   XfstCompiler& define(const char* name, const char* xre);
>   XfstCompiler& define(const char* name);
>   XfstCompiler& define_function(const char* prototype, const char* xre);
>   XfstCompiler& undefine(const char* name_list);
>   XfstCompiler& unlist(const char* name);
>   XfstCompiler& load_definitions(const char* infilename);
>   XfstCompiler& apropos(const char* text);
>   XfstCompiler& describe(const char* text);
>   XfstCompiler& clear();
>   XfstCompiler& pop();
>   XfstCompiler& push(const char* name);
>   XfstCompiler& push();
>   XfstCompiler& turn();
>   XfstCompiler& rotate();
>   XfstCompiler& load_stack(const char* infilename);
>   XfstCompiler& collect_epsilon_loops();
>   XfstCompiler& compact_sigma();
>   XfstCompiler& eliminate_flag(const char* name);
>   XfstCompiler& eliminate_flags();
>   XfstCompiler& echo(const char* text);
>   XfstCompiler& quit(const char* message);
>   XfstCompiler& system(const char* command);
>   XfstCompiler& set(const char* name, const char* text);
>   XfstCompiler& set(const char* name, unsigned int number);
>   XfstCompiler& show(const char* name);
>   XfstCompiler& show();
>   XfstCompiler& twosided_flags();
>   XfstCompiler& test_uni(Level level, bool assertion=false);
>   XfstCompiler& test_eq(bool assertion=false);
>   XfstCompiler& test_funct(bool assertion=false);
>   XfstCompiler& test_id(bool assertion=false);
>   XfstCompiler& test_upper_bounded(bool assertion=false);
>   XfstCompiler& test_upper_uni(bool assertion=false);
>   XfstCompiler& test_lower_bounded(bool assertion=false);
>   XfstCompiler& test_lower_uni(bool assertion=false);
>   XfstCompiler& test_nonnull(bool assertion=false);
>   XfstCompiler& test_null(bool invert_test_result=false, bool assertion=false);
>   XfstCompiler& test_overlap(bool assertion=false);
>   XfstCompiler& test_sublanguage(bool assertion=false);
>   XfstCompiler& test_unambiguous(bool assertion=false);
>   XfstCompiler& test_infinitely_ambiguous(bool assertion=false);
>   XfstCompiler& substitute_named(const char* variable, const char* label);
>   XfstCompiler& substitute_label(const char* list, const char* target);
>   XfstCompiler& substitute_symbol(const char* list, const char* target);
>   XfstCompiler& print_aliases(std::ostream * oss);
>   XfstCompiler& print_arc_count(const char* level, std::ostream * oss);
>   XfstCompiler& print_arc_count(std::ostream * oss);
>   XfstCompiler& print_defined(std::ostream * oss);
>   XfstCompiler& print_dir(const char* glob, std::ostream * oss);
>   XfstCompiler& print_file_info(std::ostream * oss);
>   XfstCompiler& print_flags(std::ostream * oss);
>   XfstCompiler& print_labels(std::ostream * oss, HfstTransducer* tr);
>   XfstCompiler& print_labels(const char* name, std::ostream * oss);
>   XfstCompiler& print_labels(std::ostream * oss);
>   XfstCompiler& print_labelmaps(std::ostream * oss);
>   XfstCompiler& print_label_count(std::ostream * oss);
>   XfstCompiler& print_list(const char* name, std::ostream * oss);
>   XfstCompiler& print_list(std::ostream * oss);
>   XfstCompiler& shortest_string (const hfst::HfstTransducer* transducer, hfst::HfstTwoLevelPaths& paths);
>   XfstCompiler& print_shortest_string(std::ostream * oss);
>   XfstCompiler& print_shortest_string_size(std::ostream * oss);
>   XfstCompiler& print_longest_string(std::ostream * oss);
>   XfstCompiler& print_longest_string_size(std::ostream * oss);
>   XfstCompiler& print_lower_words(const char * name, unsigned int number, std::ostream * oss);
>   XfstCompiler& print_random_lower(const char * name, unsigned int number, std::ostream * oss);
>   XfstCompiler& print_upper_words(const char * name, unsigned int number, std::ostream * oss);
>   XfstCompiler& print_random_upper(const char * name, unsigned int number, std::ostream * oss);
>   XfstCompiler& print_words(const char * name, unsigned int number, std::ostream * oss);
>   XfstCompiler& print_random_words(const char * name, unsigned int number, std::ostream * oss);
>   XfstCompiler& print_name(std::ostream * oss);
>   XfstCompiler& view_net();
>   XfstCompiler& print_net(std::ostream * oss);
>   XfstCompiler& print_net(const char* name, std::ostream * oss);
>   XfstCompiler& print_properties(std::ostream * oss);
>   XfstCompiler& print_properties(const char* name, std::ostream * oss);
>   XfstCompiler& print_sigma(const char* name, std::ostream * oss);
>   XfstCompiler& print_sigma(std::ostream * oss, bool prompt=true);
>   XfstCompiler& print_sigma_count(std::ostream * oss);
>   XfstCompiler& print_sigma_word_count(const char* level, std::ostream * oss);
>   XfstCompiler& print_sigma_word_count(std::ostream * oss);
>   XfstCompiler& print_size(const char* name, std::ostream * oss);
>   XfstCompiler& print_size(std::ostream * oss);
>   XfstCompiler& print_stack(std::ostream * oss);
>   XfstCompiler& write_dot(const char* name, std::ostream * oss);
>   XfstCompiler& write_dot(std::ostream * oss);
>   XfstCompiler& write_prolog(std::ostream * oss);
>   XfstCompiler& write_spaced(std::ostream * oss);
>   XfstCompiler& write_text(std::ostream * oss);
>   XfstCompiler& write_function(const char* name, const char* outfilename);
>   XfstCompiler& write_definition(const char* name, const char* outfilename);
>   XfstCompiler& write_definitions(const char* outfilename);
>   XfstCompiler& write_stack(const char* outfilename);
>   XfstCompiler& read_props(FILE* infile);
>   XfstCompiler& read_props(const char* indata);
>   XfstCompiler& read_regex(FILE* infile);
>   XfstCompiler& read_regex(const char* indata);
>   XfstCompiler& read_prolog(FILE* infile);
>   XfstCompiler& read_prolog(const char* indata);
>   XfstCompiler& read_spaced_from_file(const char * filename);
>   XfstCompiler& read_spaced(const char* indata);
>   XfstCompiler& read_text_from_file(const char * filename);
>   XfstCompiler& read_text(const char* indata);
>   XfstCompiler& read_lexc_from_file(const char * filename);
>   XfstCompiler& read_lexc(const char* indata);
>   XfstCompiler& read_att_from_file(const char * filename);
>   XfstCompiler& write_att(std::ostream * oss);
>   XfstCompiler& cleanup_net();
>   XfstCompiler& complete_net();
>   XfstCompiler& compose_net();
>   XfstCompiler& concatenate_net();
>   XfstCompiler& crossproduct_net();
>   XfstCompiler& determinize_net();
>   XfstCompiler& epsilon_remove_net();
>   XfstCompiler& ignore_net();
>   XfstCompiler& intersect_net();
>   XfstCompiler& invert_net();
>   XfstCompiler& label_net();
>   XfstCompiler& lower_side_net();
>   XfstCompiler& upper_side_net();
>   XfstCompiler& minimize_net();
>   XfstCompiler& minus_net();
>   XfstCompiler& name_net(const char* name);
>   XfstCompiler& negate_net();
>   XfstCompiler& one_plus_net();
>   XfstCompiler& zero_plus_net();
>   XfstCompiler& prune_net();
>   XfstCompiler& reverse_net();
>   XfstCompiler& shuffle_net();
>   XfstCompiler& sigma_net();
>   XfstCompiler& sort_net();
>   XfstCompiler& substring_net();
>   XfstCompiler& union_net();
>   XfstCompiler& inspect_net();
>   XfstCompiler& optional_net();
>   XfstCompiler& compile_replace_net(Level level);
>   XfstCompiler& compile_replace_lower_net();
>   XfstCompiler& compile_replace_upper_net();
>   XfstCompiler& compile_regex(const char * indata, unsigned int & chars_read);
>   XfstCompiler& hfst(const char * data);
>   const std::stack<HfstTransducer*>& get_stack() const;
>   XfstCompiler& setReadline(bool readline);
>   XfstCompiler& setReadInteractiveTextFromStdin(bool value);
>   XfstCompiler& setOutputToConsole(bool value);
>   XfstCompiler& setVerbosity(bool verbosity);
>   XfstCompiler& setPromptVerbosity(bool verbosity);
>   const XfstCompiler& prompt();
>   XfstCompiler& setRestrictedMode(bool value);
>   std::ostream & get_error_stream();
>   std::ostream & get_output_stream();
>   std::ostream & output();
>   std::ostream & error();
>   XfstCompiler& print_one_string_or_its_size (std::ostream * oss, const HfstTwoLevelPaths & paths, const char * level, bool print_size);
>   XfstCompiler& print_longest_string_or_its_size(std::ostream * oss, bool print_size);
>   XfstCompiler& print_words(const char * name, unsigned int number, std::ostream * oss, Level level);
>   XfstCompiler& read_text_or_spaced(const char * filename, bool spaces);
>   XfstCompiler& load_stack_or_definitions(const char *infilename, bool definitions);
>   XfstCompiler& add_loaded_definition(HfstTransducer * t);
>   XfstCompiler& apply(FILE* infile, ApplyDirection direction);
>   XfstCompiler& apply_unary_operation(UnaryOperation operation);
>   XfstCompiler& apply_binary_operation(BinaryOperation operation);
>   XfstCompiler& apply_binary_operation_iteratively(BinaryOperation operation);
>   XfstCompiler& test_operation(TestOperation operation, bool assertion=false);
>   const XfstCompiler& error(const char* message) const;
>   XfstCompiler& print_transducer_info();
>   XfstCompiler& add_prop_line(char* line);
>   XfstCompiler& lookup(char* line, const HfstTransducer * t, size_t cutoff);
>   XfstCompiler& lookup(char* line, HfstBasicTransducer * t);
>   XfstCompiler& apply_up_line(char* line);
>   XfstCompiler& apply_down_line(char* line);
>   XfstCompiler& apply_med_line(char* line);
>   XfstCompiler& print_bool(bool value);
>   bool use_readline_;
>   bool read_interactive_text_from_stdin_;
>   bool output_to_console_;
>   hfst::xre::XreCompiler xre_;
>   hfst::lexc::LexcCompiler lexc_;
>   std::map<std::string,std::string> original_definitions_;
>   std::map<std::string,hfst::HfstTransducer*> definitions_;
>   std::map<std::string,std::string> original_function_definitions_;
>   std::map<std::string,std::string> function_definitions_;
>   std::map<std::string,unsigned int> function_arguments_;
>   std::stack<hfst::HfstTransducer*> stack_;
>   std::map<std::string,hfst::HfstTransducer*> names_;
>   std::map<std::string,std::string> aliases_;
>   std::map<std::string,std::string> variables_;
>   std::map<std::string,std::string> properties_;
>   std::map<std::string,std::set<std::string> > lists_;
>   hfst::ImplementationType format_;
>   bool verbose_;
>   bool verbose_prompt_;
>   hfst::HfstTransducer * latest_regex_compiled;
>   bool quit_requested_;
>   bool fail_flag_;
>   std::ostream * output_;
>   std::ostream * error_;
>   bool restricted_mode_;
> }

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.can-arc-be-followed-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.can-arc-be-followed-fn]
> `can_arc_be_followed(int number, unsigned int number_of_arcs) -> bool`, used
> by `inspect_net` to validate a user-entered arc choice. If `number == EOF` or
> `number == 0`, print "could not read arc number" to `output()`, flush, return
> false. Else if `number < 1` or `number > (int)number_of_arcs`, print (and
> flush) either "state has no arcs" (when `number_of_arcs < 1`) or
> "arc number must be between 1 and <number_of_arcs>", and return false.
> Otherwise return true.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.can-level-be-reached-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.can-level-be-reached-fn]
> `can_level_be_reached(int level, size_t whole_path_length) -> bool`, used by
> `inspect_net` to validate a user-entered level number. If `level == EOF` or
> `level == 0`, print "could not read level number (type '0' if you wish to
> exit program)" to `output()`, flush, return false. Else if `level < 0` or
> `level > (int)whole_path_length`, print (and flush) "no such level: '<level>'
> (current level is <whole_path_length>)" and return false. Otherwise return
> true.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.check-filename-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.check-filename-fn]
> `check_filename(const char* filename) -> bool`. Enforces restricted-mode file
> access. If `restricted_mode_` is true, copy `filename` into a std::string and
> check whether it contains a `/` or `\\`: if so, print to `error()` a two-line
> message that restricted mode (--restricted-mode) only allows read/write in the
> current directory (filenames cannot contain '/' or '\\'), flush error, call
> `xfst_lesser_fail()`, call `prompt()`, and return false. In all other cases
> (not restricted, or no path separators present), call `prompt()` and return
> true. Note `prompt()` is invoked on every path before returning.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.convert-to-common-format-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.convert-to-common-format-fn]
> `convert_to_common_format(HfstTransducer* t, const char* filename=NULL)` ->
> void. Ensures `t` is in the compiler's working type `format_`. First call
> `check_filename(filename)`; if it returns false, return immediately. If
> `t->get_type() == format_`, do nothing. Otherwise: if `t`'s type is
> `HFST_OL_TYPE` or `HFST_OLW_TYPE` (optimized lookup), print (when `verbose_`)
> a warning to `error()` that the transducer is optimized-lookup and only
> 'apply up' is supported, flush, and return WITHOUT converting. For any other
> mismatched type: when `verbose_`, print to `error()` a warning "converting
> transducer type from <from-format> to <to-format>" (using
> `implementation_type_to_format`), appending " when reading from file
> '<to_filename(filename)>'" if `filename != NULL`, and appending " (loss of
> information is possible)" if `HfstTransducer::is_safe_conversion(from, format_)`
> is false; each segment is flushed. Then call `t->convert(format_)` to convert
> the transducer in place.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.current-history-index-fn]
> int

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.current-history-index-fn]
> `current_history_index() -> int`. If built with readline (`HAVE_READLINE`),
> return the readline global `history_length` (the current number of entries in
> the input history). Otherwise return -1. No side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.define-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.define-fn]
> `define(const char* name, HfstTransducer* transducer)` -> void. Registers a
> named transducer definition. Step 1: `was_defined = xre_.is_definition(name)`
> records whether the name already existed in the regex compiler. Step 2:
> `xre_.define(name, *transducer)` (re)defines it in the XreCompiler. Step 3:
> if variable `name-nets` == "ON", call `transducer->set_name(name)`. Step 4:
> look up `name` in the `definitions_` map; if present, `delete` the old
> transducer pointer and erase the entry. Step 5: store
> `definitions_[name] = transducer` (the compiler takes ownership of the passed
> pointer). Step 6: when `verbose_`, print "Redefined '<name>'" if it was
> already defined, else "Defined '<name>'", to `output()`, then flush.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
> `flush(std::ostream* oss)` -> void. On non-Windows builds this is a no-op
> (entire body is `#ifdef WINDOWS`-guarded). On Windows, when
> `output_to_console_` is true: if `oss` is the member `winoss_stderr_` buffer,
> write its accumulated string to the stderr console via
> `hfst_fprintf_console(stderr, ...)` then clear the buffer (`.str("")`) and
> return; if `oss` is the member `winoss_stdout_` buffer, do the same for the
> stdout console and clear it. Otherwise (or when not outputting to console) it
> does nothing. Used to push buffered console output after writes on Windows.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-apply-prompt-fn]
> const char *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-apply-prompt-fn]
> `get_apply_prompt(ApplyDirection direction) -> const char*`. If `!verbose_`,
> return the empty string `""`. Otherwise: if `direction == APPLY_UP_DIRECTION`
> return "apply up> "; if `direction == APPLY_DOWN_DIRECTION` return
> "apply down> "; for any other direction return "". Returns a string literal,
> no allocation or side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-fail-flag-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-fail-flag-fn]
> `get_fail_flag() const -> bool`. Trivial getter: returns the member
> `fail_flag_`. No side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-fn]
> std::string

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-fn]
> `get(const char* name) -> std::string`. Looks up `name` in the `variables_`
> map. If not present, return an empty string `""`. Otherwise return a copy of
> the stored value string `variables_[name]`. No side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-output-to-console-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-output-to-console-fn]
> `getOutputToConsole() -> bool`. Trivial getter: returns the member
> `output_to_console_`. No side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-precision-fn]
> int

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-precision-fn]
> `get_precision() -> int`. Constructs an `istringstream` over the value of
> variable `variables_["precision"]` and extracts a single `int` via
> `iss >> retval`, returning it. Standard stream parsing (leading whitespace
> skipped, stops at first non-digit; on extraction failure `retval` is
> default/uninitialized as per the stream). No error reporting. Used to set the
> ostream precision when printing weights.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-print-symbol-fn]
> const char *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-print-symbol-fn]
> `get_print_symbol(const char* symbol) -> const char*`. Maps an internal
> symbol to its printable form. Steps: (1) if variable `show-flags` == "OFF" and
> `FdOperation::is_diacritic(symbol)` is true (symbol is a flag diacritic),
> return "" (print nothing). (2) if `symbol` equals `hfst::internal_epsilon`,
> return "". (3) if `symbol` equals `hfst::internal_unknown` or
> `hfst::internal_identity`, return "?". (4) otherwise return `symbol`
> unchanged. Returns string literals or the input pointer; no allocation.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-prompt-fn]
> char *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-prompt-fn]
> `get_prompt() const -> char*`. Formats the interactive prompt into a local
> 256-byte buffer via `sprintf(p, "hfst[<n>]: ", stack_.size())` where `<n>` is
> the current stack depth (formatted with the platform size_t specifier), then
> returns `strdup(p)` — a heap-allocated copy the caller owns and must free.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-read-interactive-text-from-stdin-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-read-interactive-text-from-stdin-fn]
> `getReadInteractiveTextFromStdin() -> bool`. Trivial getter: returns the
> member `read_interactive_text_from_stdin_`. No side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-readline-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-readline-fn]
> `getReadline() -> bool`. Trivial getter: returns the member `use_readline_`.
> No side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-restricted-mode-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-restricted-mode-fn]
> `getRestrictedMode() const -> bool`. Trivial getter: returns the member
> `restricted_mode_`. No side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-stream-fn]
> std::ostream *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-stream-fn]
> `get_stream(std::ostream* oss) -> std::ostream*`. Selects the actual output
> stream to use. On non-Windows builds it just returns `oss` unchanged. On
> Windows, when `output_to_console_` is true: if `oss == &std::cerr` return the
> member console-backed buffer `&winoss_stderr_`; if `oss == &std::cout` return
> `&winoss_stdout_`. Otherwise return `oss` unchanged. Lets console output on
> Windows be redirected through buffered streams that `flush` later drains.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.ignore-history-after-index-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.ignore-history-after-index-fn]
> `ignore_history_after_index(int index)` -> void. Readline-only (entire body
> `#ifdef HAVE_READLINE`); a no-op without readline. Removes input-history
> entries added after `index`: loops `i` from `history_length - 1` down while
> `i > index - 1` (i.e. down to and including index `index`), calling
> `remove_history(i)` each iteration. Net effect: truncates the readline history
> back to length `index`.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.open-hfst-input-stream-fn]
> HfstInputStream *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.open-hfst-input-stream-fn]
> `open_hfst_input_stream(const char* infilename) -> HfstInputStream*`. Opens a
> transducer input stream for a file. Steps: (1) `assert(infilename != NULL)`.
> (2) call `check_filename(infilename)`; if false, return NULL. (3) probe the
> file by `hfst::hfst_fopen(infilename, "r")`: if it returns NULL, print
> "Could not open file <infilename>" to error(), flush, `xfst_fail()`, return
> NULL; if `fclose` of that probe handle fails (!=0), print "Could not close
> file <infilename>", flush, `xfst_fail()`, return NULL. (4) construct the real
> stream in a try block: `new HfstInputStream(infilename)` (since infilename is
> non-null this branch is taken; a `new HfstInputStream()` stdin branch exists
> but is unreachable here). (5) if a `NotTransducerStreamException` is thrown,
> print "Unable to read transducers from <to_filename(infilename)>" to error(),
> flush, `xfst_fail()`, return NULL. (6) return the new `HfstInputStream*`
> (caller owns it).

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.parse-fn]
> int

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.parse-fn]
> `parse(const char* filename) -> int`. Parses xfst commands from a named file
> via the Bison/Flex parser. Steps: (1) call `check_filename(filename)`; if
> false, return -1. (2) open the file with `hfst::hfst_fopen(filename, "r")` and
> assign to the global parser input `hxfstin`; if NULL, print "could not open
> <filename> for reading" to error(), flush, return -1. (3) set the global
> `xfst_ = this` so parser actions target this compiler. (4) call the generated
> `hxfstparse()` and capture its return value `rv`. (5) `fclose(hxfstin)`.
> (6) return `rv`. (The sibling `parse(FILE*)` overload skips the open/close and
> just sets `hxfstin`/`xfst_` and runs `hxfstparse()`.)

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.parse-line-fn]
> int

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.parse-line-fn]
> `parse_line(char line[]) -> int`. Parses a single line of xfst commands from
> an in-memory string. Steps: set global parser input `hxfstin = NULL` (no file)
> and `xfst_ = this`; create a Flex string buffer
> `bs = hxfst_scan_string(line)`; run `hxfstparse()` and capture `rv`; delete
> the buffer via `hxfst_delete_buffer(bs)`; return `rv`. (A `std::string`
> overload first `strdup`s the line into a mutable C-string, scans/parses it the
> same way, then `free`s the duplicate before returning `rv`.) Note: no prompt
> is issued (the conditional prompt-on-error is commented out).

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-alphabet-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-alphabet-fn]
> `print_alphabet(const StringSet& alpha, bool unknown, bool identity,
> std::ostream* oss_)` -> void. Prints the sigma (alphabet). Resolve the real
> stream `oss = get_stream(oss_)`; init `sigma_count = 0`; write "Sigma: ".
> If variable `print-foma-sigma` == "ON": if `unknown` write "?"; if `identity`
> write ", " (only when `unknown` was also set) followed by "@". Otherwise
> (xfst-style): if `unknown || identity` write a single "?". Then iterate
> `alpha` with a `first_symbol` flag: skip special symbols (`is_special_symbol`);
> for each ordinary symbol, write ", " if `!first_symbol || unknown || identity`
> (separator), then write the symbol — but write `"?"` quoted if the symbol is
> literally `?`, write `"@"` quoted if the symbol is `@` and `print-foma-sigma`
> is "ON", else write the symbol verbatim; increment `sigma_count` and clear
> `first_symbol`. After the loop write a newline, then "Size: <sigma_count>."
> and a newline, and flush.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-arcs-fn]
> unsigned int

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-arcs-fn]
> `print_arcs(const HfstBasicTransitions& transitions) -> unsigned int`, used by
> `inspect_net` to list the outgoing arcs of a state. Track `arc_number = 1` and
> a `first_loop` flag. For each transition: on the first one write "Arcs:" (and
> clear `first_loop`), on subsequent ones write ", "; flush output() each time.
> Read `isymbol = get_input_symbol()` and `osymbol = get_output_symbol()`; if
> they are equal write " <arc_number>. <isymbol>", else write
> " <arc_number>. <isymbol>:<osymbol>"; flush; increment `arc_number`. After the
> loop write a newline and flush. Return `arc_number - 1` (the count of arcs
> printed; 0 if `transitions` was empty).

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-level-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-level-fn]
> `print_level(const std::vector<unsigned int>& whole_path, const
> std::vector<unsigned int>& shortest_path)` -> void, for `inspect_net`. Writes
> "Level <whole_path.size()>" to output(); if `shortest_path.size() <
> whole_path.size()` (a loop was collapsed), additionally write
> " (= <shortest_path.size()>)". Then flush output(). No trailing newline.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-paths-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-paths-fn]
> `print_paths(const HfstTwoLevelPaths& paths, std::ostream* oss_=cout,
> int n=-1) -> bool`. Prints up to `n` two-level paths (`n == -1` means
> unlimited, since the loop condition is `n != 0`). Resolve `oss =
> get_stream(oss_)`; set `oss->precision(get_precision())`; `retval = false`.
> Iterate paths while `n != 0`: take `path = it->second` (a StringPairVector).
> If variable `obey-flags` == "ON", build the input-side StringVector via
> `symbols::to_string_vector(path, true)` and skip this path
> (`continue`, without decrementing n) when `is_valid_string` is false. Set
> `retval = true`. Then for each symbol pair `p`: compute
> `print_symbol = get_print_symbol(p.first)`; if variable `print-space` == "ON"
> and something was already printed and `print_symbol` is non-empty, write a
> space; write `print_symbol`; if it was non-empty set `something_printed`.
> Compute `print_symbol = get_print_symbol(p.second)`; if it is non-empty AND
> `p.first != p.second`, write ":" then the output symbol. After the path, if
> variable `print-weight` == "ON" write a tab then the weight `it->first` with
> `std::fixed`. Write a newline and `--n`. After the loop, flush `oss` and
> return `retval` (whether anything was printed).

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.quit-requested-fn]
> bool

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.quit-requested-fn]
> `quit_requested() const -> bool`. Trivial getter: returns the member
> `quit_requested_`. No side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.remove-newline-fn]
> char *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.remove-newline-fn]
> `remove_newline(char* line) -> char*`. Strips line-ending characters in
> place. Iterates index `i` from 0 until the NUL terminator: whenever
> `line[i]` is `'\n'` or `'\r'`, overwrite it with `'\0'`; always increment
> `i`. NOTE that overwriting with `'\0'` does not stop the loop early because
> the increment and the loop condition read the position the loop was already
> past (the loop only terminates on a NUL that was originally present), so it
> mutates every `\n`/`\r` byte up to the first original NUL. Returns the same
> `line` pointer.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
> `set_error_stream(std::ostream& os) -> void`. Sets the member `error_` to
> `&os`, then propagates that error stream to the lexc compiler via
> `this->lexc_.set_error_stream(this->error_)`. (The corresponding propagation
> to the regex compiler `xre_` is commented out in the source and not done.)

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
> `set_output_stream(std::ostream& os) -> void`. Trivial setter: assigns the
> member `output_` to `&os`. No other side effects.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.top-fn]
> HfstTransducer *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.top-fn]
> `top() -> HfstTransducer*`. Returns the top transducer of the stack, or NULL
> on error. Steps: (1) if `stack_.size() < 1`, emit "Empty stack." to `error()`
> and flush (the EMPTY_STACK macro), call `xfst_lesser_fail()`, call `prompt()`,
> and return NULL. (2) take `retval = stack_.top()`. (3) if `retval`'s type is
> `HFST_OL_TYPE` or `HFST_OLW_TYPE` (optimized lookup), print to `error()`
> "Operation not supported for optimized lookup format. Consider
> 'remove-optimization' to convert into ordinary format.", flush, call
> `prompt()`, and return NULL. (4) otherwise return `retval` (the stack is not
> modified — this is a peek, not a pop).

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.unknown-command-fn]
> int

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.unknown-command-fn]
> `unknown_command(const char* s) -> int`. Reports an unrecognised command.
> If variable `quit-on-fail` == "ON": when `verbose_` is true, print "Command
> <s> is not recognised." to `error()` and flush; then return 1 (signalling a
> hard failure, regardless of verbosity). Otherwise (`quit-on-fail` != "ON"):
> always print "Command <s> is not recognised." to `error()` and flush, call
> `this->prompt()`, and return 0.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-compiler-fn]
> XfstCompiler::XfstCompiler()

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-compiler-fn]
> `XfstCompiler::XfstCompiler()` default constructor. Initializer list sets:
> `use_readline_=false`, `read_interactive_text_from_stdin_=false`,
> `output_to_console_=false`; constructs `xre_`, `lexc_`, and sets `format_`
> all to `hfst::TROPICAL_OPENFST_TYPE`; `verbose_=false`,
> `verbose_prompt_=false`, `latest_regex_compiled=NULL`,
> `quit_requested_=false`, `fail_flag_=false`, `output_=&std::cout`,
> `error_=&std::cerr` (on Windows also the two console ostringstream buffers),
> `restricted_mode_=false`. Body: configure `xre_` —
> `set_expand_definitions(true)`, `set_verbosity(verbose_)`,
> `set_flag_harmonization(false)`, `set_error_stream(error_)`. Configure
> `lexc_` — `setVerbosity(verbose_ ? 2 : 0)`, `set_error_stream(error_)`. Call
> `hfst::set_xerox_composition(true)`. Then populate the `variables_` map with
> the default settings (each value is a string): assert="OFF",
> att-epsilon="@0@ | @_EPSILON_SYMBOL_@", char-encoding="UTF-8",
> copyright-owner="Copyleft (c) UiT The Arctic University of Norway" (PORT
> DIVERGENCE: upstream's default named the University of Helsinki; the port
> names its own copyright holder, per
> [spec:hfst:sem:hfst-commandline.print-version-fn]), directory="OFF",
> encode-weights="OFF", flag-is-epsilon="OFF", harmonize-flags="OFF",
> hopcroft-min="ON", lexc-minimize-flags="OFF", lexc-rename-flags="OFF",
> lexc-with-flags="OFF", lookup-cycle-cutoff="5", maximum-weight="OFF",
> minimal="ON", name-nets="OFF", obey-flags="ON", precision="5",
> print-foma-sigma="OFF", print-pairs="OFF", print-sigma="OFF",
> print-space="OFF", print-weight="OFF", print-words-cycle-cutoff="5",
> quit-on-fail="OFF", quote-special="OFF", random-seed="ON",
> recode-cp1252="NEVER", recursive-define="OFF", retokenize="ON",
> show-flags="OFF", sort-arcs="MAYBE", use-timer="OFF", verbose="OFF",
> xerox-composition="ON". Finally call `initialize_variable_explanations()`
> and `prompt()`. (The `ImplementationType` overload is identical except every
> use of TROPICAL_OPENFST_TYPE is replaced by the passed `impl`.)

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fail-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fail-fn]
> `xfst_fail() -> void`. If variable `quit-on-fail` == "ON", set member
> `fail_flag_ = true`. Otherwise do nothing. Used to mark that the application
> should terminate after a failed command.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fclose-fn]
> int

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fclose-fn]
> `xfst_fclose(FILE* f, const char* name) -> int`. Calls `fclose(f)` and stores
> `retval`. If `retval != 0` (close failed), print "could not close file <name>"
> to `error()`, flush error, and call `xfst_fail()` (which sets the fail flag
> when quit-on-fail is ON). Returns `retval` (the result of `fclose`).

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fopen-fn]
> FILE *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fopen-fn]
> `xfst_fopen(const char* path, const char* mode) -> FILE*`. Opens a file via
> `hfst::hfst_fopen(path, mode)`. If the result is NULL (open failed), print
> "could not open file <path>" to `error()`, flush error, and call `xfst_fail()`
> (sets the fail flag when quit-on-fail is ON). Returns the FILE* (possibly
> NULL).

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
> char *

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
> `xfst_getline(FILE* file, const std::string& promptstr) -> char*`. Reads one
> line of input, returning a heap-allocated C string the caller owns, or NULL on
> EOF/error. Branches: (1) when built with readline (`HAVE_READLINE`) AND
> `use_readline_` is true AND `file == stdin`: bind tab to `rl_insert` (disable
> autocomplete), call `readline(promptstr)`; if its result is non-NULL and its
> first char is not `'\0'`, call `add_history(buf)`; return `buf`. (2) Otherwise:
> print `promptstr` to `output()` and flush. On Windows, if `file == stdin` and
> `read_interactive_text_from_stdin_`, read a line from the console via
> `hfst::get_line_from_console(str, 1000)`; return `strdup(str.c_str())` on
> success or NULL on failure. (3) Fall-through (default path): call
> `getline(&line_, &len, file)` with `line_=0`, `len=1024`; if it returns -1
> (EOF/error) return NULL, otherwise return the buffer `getline` allocated.

> [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-lesser-fail-fn]
> void

> [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-lesser-fail-fn]
> `xfst_lesser_fail() -> void`. If variable `quit-on-fail` == "ON" AND
> `read_interactive_text_from_stdin_` is false, set member `fail_flag_ = true`.
> Otherwise do nothing. Differs from `xfst_fail` by additionally suppressing the
> failure when reading interactive text from stdin.

> [spec:hfst:def:xfst-compiler.hxfst-delete-buffer-fn]
> extern void hxfst_delete_buffer(YY_BUFFER_STATE)

> [spec:hfst:sem:xfst-compiler.hxfst-delete-buffer-fn]
> `extern void hxfst_delete_buffer(YY_BUFFER_STATE)`. External declaration only
> (no body in this translation unit); the definition is generated by flex for
> the `hxfst` scanner. Frees/destroys a scanner buffer previously created by
> `hxfst_scan_string`. Called after parsing a string buffer to release it.

> [spec:hfst:def:xfst-compiler.hxfst-scan-string-fn]
> extern YY_BUFFER_STATE hxfst_scan_string(const char *)

> [spec:hfst:sem:xfst-compiler.hxfst-scan-string-fn]
> `extern YY_BUFFER_STATE hxfst_scan_string(const char*)`. External declaration
> only (no body in this translation unit); the definition is generated by flex
> for the `hxfst` scanner. Sets up the scanner to read its tokens from the given
> NUL-terminated C string instead of a FILE, returning a buffer-state handle that
> must later be freed with `hxfst_delete_buffer`. Used to feed an in-memory
> command string to the xfst parser.

> [spec:hfst:def:xfst-compiler.hxfstlex-fn]
> extern int hxfstlex(void)

> [spec:hfst:sem:xfst-compiler.hxfstlex-fn]
> `extern int hxfstlex(void)`. External declaration only (no body in this
> translation unit); the definition is generated by flex for the `hxfst`
> scanner. It is the scanner's token-producing function: reads from the current
> input source (FILE `hxfstin` or a scan-string buffer) and returns the next
> token id (0 at end of input), invoked by the generated parser `hxfstparse`.

> [spec:hfst:def:xfst-compiler.hxfstparse-fn]
> extern int hxfstparse(void)

> [spec:hfst:sem:xfst-compiler.hxfstparse-fn]
> `extern int hxfstparse(void)`. External declaration only (no body in this
> translation unit); the definition is generated by bison for the `hxfst`
> grammar. Runs the xfst command parser, pulling tokens from `hxfstlex` and
> executing the grammar actions (which drive the XfstCompiler), reading input
> from FILE `hxfstin` or a previously installed scan-string buffer. Returns 0 on
> a successful parse, nonzero on parse error.

> [spec:hfst:def:xfst-compiler.main-fn]
> int

> [spec:hfst:sem:xfst-compiler.main-fn]
> `main(int argc, char** argv) -> int`. Compiled only under `DEBUG_MAIN`; a
> placeholder unit-test driver. Prints "Unit tests for <file>:" then a
> "constructors:" line to std::cout, exercising the available constructors: a
> default `XfstCompiler` (note the source line `XfstCompiler defaultXfst();` is
> actually a function declaration, not an object — most-vexing-parse), and,
> guarded by the `HAVE_SFST`/`HAVE_OPENFST` build macros, instances for
> SFST_TYPE, TROPICAL_OPENFST_TYPE, and FOMA_TYPE, printing each backend name.
> No real tests are run (a FIXME notes this). Returns `EXIT_SUCCESS`.

> [spec:hfst:def:xfst-compiler.yy-buffer-state]
> typedef yy_buffer_state *YY_BUFFER_STATE

