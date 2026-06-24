# libhfst/src/implementations/optimized-lookup/pmatch.cc, libhfst/src/implementations/optimized-lookup/pmatch.h

> [spec:hfst:def:pmatch.hfst-ol.capture]
> struct Capture {
>   unsigned int begin;
>   unsigned int end;
>   SymbolNumber name;
> }

> [spec:hfst:def:pmatch.hfst-ol.context-matched-trap]
> struct ContextMatchedTrap {
>   bool polarity;
> }

> [spec:hfst:def:pmatch.hfst-ol.context-matched-trap.context-matched-trap-fn]
> explicit ContextMatchedTrap(bool p): polarity(p)

> [spec:hfst:sem:pmatch.hfst-ol.context-matched-trap.context-matched-trap-fn]
> Explicit constructor for ContextMatchedTrap. Stores the bool argument `p`
> into the `polarity` member. No other side effects.

> [spec:hfst:def:pmatch.hfst-ol.counter-comp-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.counter-comp-fn]
> Free comparison function used to sort (name, count) pairs in descending
> order of count. Takes two std::pair<std::string, unsigned long> values `l`
> and `r` by value and returns `l.second > r.second`. The string `.first`
> members are ignored.

> [spec:hfst:def:pmatch.hfst-ol.location]
> struct Location {
>   unsigned int start;
>   unsigned int length;
>   std::string input;
>   std::string middle;
>   std::string output;
>   std::string tag;
>   Weight weight;
>   std::vector<size_t> input_parts;
>   std::vector<size_t> output_parts;
>   std::vector<std::string> input_symbol_strings;
>   std::vector<std::string> output_symbol_strings;
> }

> [spec:hfst:def:pmatch.hfst-ol.location-vector]
> typedef std::vector<Location> LocationVector

> [spec:hfst:def:pmatch.hfst-ol.location-vector-vector]
> typedef std::vector<LocationVector> LocationVectorVector

> [spec:hfst:def:pmatch.hfst-ol.location.operator-fn]
> bool operator<(Location rhs) const

> [spec:hfst:sem:pmatch.hfst-ol.location.operator-fn]
> Less-than operator on Location, taking `rhs` by value. Returns
> `this->weight < rhs.weight`. Compares only by the `weight` member; all
> other members are ignored. Const member function with no side effects.

> [spec:hfst:def:pmatch.hfst-ol.n-byte-grapheme-fn]
> int

> [spec:hfst:sem:pmatch.hfst-ol.n-byte-grapheme-fn]
> Free function `nByte_grapheme(const char *u8)`. Returns the number of bytes
> occupied by the first grapheme cluster of the UTF-8 string `u8`, using ICU.
> Steps: (1) allocate a UChar buffer of size strlen(u8)+1 and convert the
> whole UTF-8 string to UTF-16 with u_strFromUTF8, recording the UTF-16
> `length`; on ICU failure print an error to stderr but continue. (2) Open a
> UBRK_CHARACTER (grapheme) break iterator for locale "C" via ubrk_open
> (errors printed to stderr). (3) ubrk_setText it over the UChar buffer
> (errors printed to stderr). (4) `begin = ubrk_first()`, `end =
> ubrk_next()`. (5) If begin == end, return 0. (6) If end == UBRK_DONE,
> return 0. (7) Otherwise allocate a char buffer of size (end-begin)*4+1,
> convert the UTF-16 range [begin,end) back to UTF-8 via u_strToUTF8 (errors
> printed to stderr), and return strlen of the resulting UTF-8 grapheme (its
> byte count). Note: allocations are made with malloc and are not freed
> (leaks ICUdata and grapheme); the break iterator is not closed.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet]
> class PmatchAlphabet: public TransducerAlphabet {
>   RtnVector rtns;
>   SymbolNumber input_mark_symbol = 0;
>   SymbolNumberVector special_symbols;
>   std::map<SymbolNumber, std::string> end_tag_map;
>   std::map<std::string, SymbolNumber> capture_tag_map;
>   std::map<std::string, SymbolNumber> captured_tag_map;
>   SymbolNumberVector capture2captured;
>   SymbolNumberVector captured2capture;
>   RtnNameMap rtn_names;
>   SymbolNumberVector symbol2lists;
>   SymbolNumberVector list2symbols;
>   SymbolNumberVector exclusionary_lists;
>   std::vector<SymbolNumberVector> symbol_lists;
>   std::vector<SymbolNumberVector> symbol_list_members;
>   std::vector<unsigned long> counters;
>   SymbolNumberVector guards;
>   std::vector<bool> global_flags;
>   std::vector<bool> printable_vector;
>   PmatchContainer * container;
> }

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-rtn-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-rtn-fn]
> `add_rtn(PmatchTransducer *rtn, std::string const &name)`. Looks up the
> SymbolNumber for `name` in `rtn_names` (map<string,SymbolNumber>; missing
> keys are default-inserted as 0 by operator[]) and stores `rtn` into
> `rtns[symbol]`. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-special-symbol-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-special-symbol-fn]
> `add_special_symbol(const std::string &str, SymbolNumber symbol_number)`.
> Classifies the special symbol string `str` and records `symbol_number`
> appropriately. An if/else-if chain on exact string equality:
> "@PMATCH_ENTRY@"->special_symbols[entry], "@PMATCH_EXIT@"->[exit],
> "@PMATCH_LC_ENTRY@"->[LC_entry], "@PMATCH_RC_ENTRY@"->[RC_entry],
> "@PMATCH_LC_EXIT@"->[LC_exit], "@PMATCH_RC_EXIT@"->[RC_exit],
> "@PMATCH_NLC_ENTRY@"->[NLC_entry], "@PMATCH_NRC_ENTRY@"->[NRC_entry],
> "@PMATCH_NLC_EXIT@"->[NLC_exit], "@PMATCH_NRC_EXIT@"->[NRC_exit],
> "@PMATCH_PASSTHROUGH@"->[Pmatch_passthrough], "@BOUNDARY@"->[boundary],
> "@UNICODE_ALPHA@"->[UnicodeAlpha], "@UNICODE_UPPERALPHA@"->[UnicodeUpperAlpha],
> "@UNICODE_LOWERALPHA@"->[UnicodeLowerAlpha],
> "@UNICODE_WHITESPACE@"->[UnicodeWhitespace] (each assigns symbol_number into
> that special_symbols slot). Else if is_end_tag(str): extract the substring
> between the "@PMATCH_ENDTAG_" prefix and the trailing "@" and store it in
> end_tag_map[symbol_number]. Else if is_capture_tag(str): extract the name
> between "@PMATCH_CAPTURE_" and "@", set capture_tag_map[name]=symbol_number;
> if captured_tag_map already has that name, link them via
> capture2captured[symbol_number]=captured_tag_map[name] and
> captured2capture[captured_tag_map[name]]=symbol_number. Else if
> is_captured_tag(str): extract name between "@PMATCH_CAPTURED_" and "@", set
> captured_tag_map[name]=symbol_number; if capture_tag_map has that name, link
> captured2capture[symbol_number]=capture_tag_map[name] and
> capture2captured[capture_tag_map[name]]=symbol_number. Else if
> is_insertion(str): rtn_names[name_from_insertion(str)]=symbol_number. Else if
> is_guard(str): push symbol_number onto `guards`. Else if
> is_underscored_list(str): call process_underscored_symbol_list(str,
> symbol_number). Else if is_list(str): call process_symbol_list(str,
> symbol_number). Else if is_counter(str): call process_counter(str,
> symbol_number). Else (fallthrough): set printable_vector[symbol_number]=true
> (treated as a regular symbol). No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-symbol-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-symbol-fn]
> `add_symbol(const std::string &symbol)`. Appends bookkeeping entries for a
> new symbol whose index will be the current symbol_table.size(). Push
> NO_SYMBOL_NUMBER onto symbol2lists, list2symbols, capture2captured,
> captured2capture; push NULL onto rtns; push true onto printable_vector. If
> `exclusionary_lists` is non-empty, the new symbol must be accepted by all
> exclusionary lists: set symbol2lists[symbol_table.size()] to
> size_t_to_ushort(symbol_lists.size()), push a copy of exclusionary_lists
> (as a SymbolNumberVector) onto symbol_lists, and for each exclusionary list
> index `exc` append symbol_table.size() to
> symbol_list_members[list2symbols[exc]]. Finally call
> TransducerAlphabet::add_symbol(symbol) (which actually appends to
> symbol_table). No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.count-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.count-fn]
> `count(SymbolNumber sym)`. If is_counter(sym) is true (sym is within
> `counters` and its slot is not NO_COUNTER), increment counters[sym] by one.
> Otherwise do nothing. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.end-tag-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.end-tag-fn]
> `end_tag(const SymbolNumber symbol)`. If `end_tag_map` has no entry for
> `symbol`, return the empty string. Otherwise return the string
> "</" + end_tag_map[symbol] + ">" (an XML-style closing tag).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-counter-name-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-counter-name-fn]
> `get_counter_name(SymbolNumber symbol)`. If symbol is out of range
> (symbol_table.size() <= symbol), return "INVALID_COUNTER". Otherwise read
> name = symbol_table[symbol]; if is_counter(name) is false, return
> "INVALID_COUNTER". Otherwise return the substring of `name` after the
> "@PMATCH_COUNTER_" prefix up to (but not including) the trailing "@" — i.e.
> name.substr(strlen("@PMATCH_COUNTER_"), name.size() -
> strlen("@PMATCH_COUNTER_") - 1).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-rtn-fn]
> PmatchTransducer *

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-rtn-fn]
> `get_rtn(SymbolNumber symbol)`. Returns the PmatchTransducer* stored at
> rtns[symbol] (no bounds check). (There is also an overload taking a string
> name, which returns rtns[rtn_names[name]].)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-special-fn]
> SymbolNumber

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-special-fn]
> `get_special(SpecialSymbol special) const`. Returns
> special_symbols.at(special) — the SymbolNumber recorded for that
> SpecialSymbol enum slot (NO_SYMBOL_NUMBER if never set). Uses bounds-checked
> vector::at.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-specials-fn]
> SymbolNumberVector

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-specials-fn]
> `get_specials() const`. Builds and returns a SymbolNumberVector containing
> every entry of `special_symbols` that is not NO_SYMBOL_NUMBER, in iteration
> order of the special_symbols vector. Entries left as NO_SYMBOL_NUMBER are
> skipped.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.has-rtn-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.has-rtn-fn]
> `has_rtn(std::string const &name) const`. If name == "TOP", return true.
> Otherwise return true iff rtn_names contains `name`, its mapped
> SymbolNumber is < rtns.size(), and rtns[that symbol] != NULL. (An overload
> taking a SymbolNumber returns symbol < rtns.size() && rtns[symbol] != NULL.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-capture-tag-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-capture-tag-fn]
> `is_capture_tag(const std::string &symbol)` (static-style string check).
> Returns true iff `symbol` begins with "@PMATCH_CAPTURE_" (find == 0) and its
> last character is "@" (rfind("@") == size-1). (A SymbolNumber overload
> instead returns capture2captured[symbol] != NO_SYMBOL_NUMBER.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-captured-tag-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-captured-tag-fn]
> `is_captured_tag(const std::string &symbol)` (string check). Returns true
> iff `symbol` begins with "@PMATCH_CAPTURED_" (find == 0) and its last
> character is "@" (rfind("@") == size-1). (A SymbolNumber overload instead
> returns captured2capture[symbol] != NO_SYMBOL_NUMBER.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-counter-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-counter-fn]
> `is_counter(const std::string &symbol)` (string check). Returns true iff
> `symbol` begins with "@PMATCH_COUNTER_" (find == 0) and its last character
> is "@" (rfind("@") == size-1). (A SymbolNumber overload instead returns
> symbol < counters.size() && counters[symbol] != NO_COUNTER.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-end-tag-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-end-tag-fn]
> `is_end_tag(const std::string &symbol)` (string check). Returns true iff
> `symbol` begins with "@PMATCH_ENDTAG_" (find == 0) and its last character is
> "@" (rfind("@") == size-1). (A SymbolNumber overload instead returns
> end_tag_map.count(symbol) == 1.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-global-flag-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-global-flag-fn]
> `is_global_flag(const std::string &symbol)` (string check). Returns true iff
> `symbol` begins with either "@P." or "@C." (find == 0 for one of them), has
> the literal "PMATCH_GLOBAL_" starting at index 3 (find("PMATCH_GLOBAL_") ==
> 3), and ends with "@" (rfind("@") == size-1). (A SymbolNumber overload
> instead returns global_flags[symbol].)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
> `is_guard(const SymbolNumber symbol) const`. Linear search of the `guards`
> vector: returns true if any element equals `symbol`, otherwise false. (A
> separate string overload `is_guard(const std::string &)` checks that the
> string begins with "@PMATCH_GUARD_" and ends with "@".)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-input-mark-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-input-mark-fn]
> `is_input_mark(const SymbolNumber symbol) const`. Returns true iff `symbol`
> equals the stored `input_mark_symbol`. (input_mark_symbol is 0 when no
> "@PMATCH_INPUT_MARK@" symbol exists.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-insertion-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-insertion-fn]
> `is_insertion(const std::string &symbol)` (string check). Returns true iff
> `symbol` begins with "@I." (find == 0) and its last character is "@"
> (rfind("@") == size-1).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-list-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-list-fn]
> `is_list(const std::string &symbol)` (string check). Returns true iff
> `symbol` begins with either "@L." or "@X." (find == 0 for one of them), its
> last character is "@" (rfind("@") == size-1), and its size is greater than
> 4.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-meta-arc-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-meta-arc-fn]
> `is_meta_arc(const SymbolNumber symbol) const`. Returns true iff
> TransducerAlphabet::is_meta_arc(symbol) is true, OR `symbol` equals any of
> the special symbol numbers get_special(UnicodeAlpha),
> get_special(UnicodeUpperAlpha), get_special(UnicodeLowerAlpha), or
> get_special(UnicodeWhitespace).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-printable-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-printable-fn]
> `is_printable(const std::string &symbol)` (string overload). If symbol.size()
> < 3, return true. Otherwise return true unless the symbol is bracketed by
> "@" on both ends — i.e. return (symbol.find("@") != 0 || symbol.at(size-1)
> != '@'). (Note there is also a SymbolNumber overload, is_printable(symbol) ->
> symbol < printable_vector.size() && printable_vector[symbol], which is a
> different function.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-special-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-special-fn]
> `is_special(const std::string &symbol)` (string check). If symbol.size() < 3,
> return false. If symbol equals "@PMATCH_INPUT_MARK@" or "@PMATCH_BACKTRACK@",
> return false (these special symbols can't be referenced in pmatch scripts).
> If is_insertion(symbol) or symbol equals "@BOUNDARY@", "@UNICODE_ALPHA@",
> "@UNICODE_UPPERALPHA@", "@UNICODE_LOWERALPHA@", or "@UNICODE_WHITESPACE@",
> return true. Otherwise return true iff (symbol begins with "@PMATCH" and its
> last character is "@") OR is_list(symbol).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-underscored-list-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-underscored-list-fn]
> `is_underscored_list(const std::string &symbol)` (string check). Returns true
> iff `symbol` begins with either "@L." or "@X." (find == 0 for one of them),
> ends with the two-character suffix "_@" (rfind("_@") == size-2), and its size
> is greater than 5.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.locatefy-fn]
> Location

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.locatefy-fn]
> `locatefy(unsigned int input_offset, const WeightedDoubleTape &str)`.
> Reconstructs a Location from a weighted double tape. Initialize retval with
> retval.start = input_offset and retval.weight = str.weight; local counters
> input_mark = 0 and output_mark = 0. Iterate each tape cell (it->input,
> it->output) in order: (a) If is_end_tag(output): when
> container->count_patterns is set, increment container->pattern_counts for
> the key start_tag(output) (initializing to 1 if absent); set retval.tag =
> start_tag(output); then `continue` (skip the rest for this cell). (b) If
> is_printable(output) (SymbolNumber overload), append
> string_from_symbol(output) to retval.output and push it onto
> retval.output_symbol_strings. (c) If is_printable(input), append
> string_from_symbol(input) to retval.input, push it onto
> retval.input_symbol_strings, and increment the local input_offset. (d) If
> is_input_mark(output): push current output_mark onto retval.output_parts and
> current input_mark onto retval.input_parts, then set output_mark =
> retval.output_symbol_strings.size() and input_mark =
> retval.input_symbol_strings.size(). After the loop: if output_mark > 0 push
> output_mark onto retval.output_parts; if input_mark > 0 push input_mark onto
> retval.input_parts. Set retval.length = input_offset - retval.start. Return
> retval.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.name-from-insertion-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.name-from-insertion-fn]
> `name_from_insertion(const std::string &symbol)`. Returns the substring of
> `symbol` after the "@I." prefix up to (but not including) the trailing "@" —
> i.e. symbol.substr(strlen("@I."), symbol.size() - strlen("@I.@")). For
> example "@I.foo@" yields "foo".

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.pmatch-alphabet-fn]
> PmatchAlphabet::PmatchAlphabet(std::istream &inputstream,

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.pmatch-alphabet-fn]
> Constructor `PmatchAlphabet(std::istream &inputstream, SymbolNumber
> symbol_count, PmatchContainer *cont)`. First delegates to base
> TransducerAlphabet(inputstream, symbol_count, true) (reading the symbol
> table), initializes `special_symbols` to a vector of SPECIALSYMBOL_NR_ITEMS
> entries all set to NO_SYMBOL_NUMBER, and stores `cont` in `container`. Then
> in the body: initialize symbol2lists, list2symbols, capture2captured,
> captured2capture each to a SymbolNumberVector of length orig_symbol_count
> filled with NO_SYMBOL_NUMBER; rtns to an RtnVector of length
> orig_symbol_count filled with NULL; printable_vector to a vector<bool> of
> length orig_symbol_count filled with false; global_flags likewise to all
> false. Then loop i from 1 to symbol_table.size()-1: if
> is_special(symbol_table[i]) call add_special_symbol(symbol_table[i], i).
> Otherwise: if symbol_table[i] == "@PMATCH_INPUT_MARK@" set input_mark_symbol
> = i; else if !is_flag_diacritic(i) set printable_vector[i] = true; else if
> is_flag_diacritic(i) and is_global_flag(symbol_table[i]) is true: set
> global_flags[i] = true, compute `feature` as
> FdOperation::get_feature(symbol_table[i]) with its leading 14 characters (the
> "PMATCH_GLOBAL_" portion) stripped via substr(14, npos), `value` as
> FdOperation::get_value(symbol_table[i]), build new_diacritic as the symbol's
> first 3 chars + feature + (value empty ? "" : "." + value) + "@", redefine
> the diacritic via fd_table.define_diacritic(i, new_diacritic) (turning the
> global flag into an ordinary non-global flag), then call
> fd_table.get_symbols_with_feature(feature) and set global_flags[*it] = true
> for every returned symbol so all flags sharing that feature are marked
> global. No return value. (There are also an alternate constructor taking a
> TransducerAlphabet const& which does the same vector setup but no
> global-flag handling, and a default constructor.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-counter-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-counter-fn]
> `process_counter(std::string str, SymbolNumber sym)`. Registers a counter for
> symbol number `sym`. `str` is accepted but unused. While `counters.size() <
> sym`, push NO_COUNTER onto `counters` (padding any gaps for non-counter symbol
> numbers below `sym`). Then push 0 onto `counters` (the new counter for `sym`,
> initialized to zero). So after the call `counters[sym] == 0`. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-symbol-list-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-symbol-list-fn]
> `process_symbol_list(const std::string &str, SymbolNumber sym)`. Builds the
> membership data for a list symbol `sym` (a "@L." inclusive list or "@X."
> exclusionary list). `polarity = (str[1] == 'L')` (true for inclusive @L lists,
> false for exclusionary @X lists). Compute begin = strlen("@L.") = 3 and stop =
> str.size() - begin - 1 (stripping the trailing "@"); call
> container->symbol_vector_from_symbols(str.substr(begin, stop)) to tokenize the
> middle into `list_symbols` (a SymbolNumberVector). For each symbol `*it` in
> list_symbols, IF polarity is true: if symbol2lists[*it] == NO_SYMBOL_NUMBER,
> set symbol2lists[*it] = size_t_to_ushort(symbol_lists.size()) and push a new
> SymbolNumberVector(1, sym) onto symbol_lists; else push `sym` onto
> symbol_lists[symbol2lists[*it]]. Set list2symbols[sym] =
> size_t_to_ushort(symbol_list_members.size()). IF polarity is false
> (exclusionary): build excl_symbols, push `sym` onto exclusionary_lists, and
> for each candidate_for_list from 1 to symbol_table.size()-1 that
> is_printable(symbol_table[candidate]) (string overload) AND is NOT in
> list_symbols (std::find == end), append candidate to excl_symbols and
> associate it with this list exactly as above (if symbol2lists[candidate] ==
> NO_SYMBOL_NUMBER create a new symbol_lists entry SymbolNumberVector(1, sym);
> else push sym onto symbol_lists[symbol2lists[sym]] — note: indexed by sym, not
> candidate, mirroring the source), then push excl_symbols onto
> symbol_list_members. ELSE (inclusive) push list_symbols onto
> symbol_list_members. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-underscored-symbol-list-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-underscored-symbol-list-fn]
> `process_underscored_symbol_list(const std::string &str, SymbolNumber sym)`.
> Like process_symbol_list but the member symbols are encoded as underscore-
> separated substrings (after the "@L."/"@X." prefix) rather than parsed via the
> encoder, and members may be brand-new symbols. Build a StringSymbolMap ss =
> build_string_symbol_map(). polarity = (str[1] == 'L'). begin = strlen("@L.") =
> 3. Loop: while stop = str.find('_', begin) != npos, take symbol =
> str.substr(begin, stop - begin); if that substring is empty (i.e. the symbol
> itself was an underscore, two underscores in a row), set symbol = "_" and begin
> = stop + 2, otherwise begin = stop + 1; push symbol onto collected_symbols.
> Then for each collected symbol string `*it`: if ss does not contain it, call
> add_symbol(*it), set str_sym = orig_symbol_count and increment
> orig_symbol_count; else str_sym = ss[*it]. Push str_sym onto list_symbols. If
> polarity is true: if symbol2lists[str_sym] == NO_SYMBOL_NUMBER set it to
> size_t_to_ushort(symbol_lists.size()) and push SymbolNumberVector(1, sym) onto
> symbol_lists, else push sym onto symbol_lists[symbol2lists[str_sym]]. After the
> loop set list2symbols[sym] = size_t_to_ushort(symbol_list_members.size()). If
> polarity is false (exclusionary): build excl_symbols, push sym onto
> exclusionary_lists, and for each candidate_for_list from 1 to
> symbol_table.size()-1 that is_printable(symbol_table[candidate]) AND is not in
> list_symbols, push candidate onto excl_symbols and associate it (if
> symbol2lists[candidate] == NO_SYMBOL_NUMBER set it to current
> symbol_lists.size() and push SymbolNumberVector(1, sym); else push sym onto
> symbol_lists[symbol2lists[candidate]]), then push excl_symbols onto
> symbol_list_members. Else push list_symbols onto symbol_list_members. No
> return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.start-tag-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.start-tag-fn]
> `start_tag(const SymbolNumber symbol)`. If `end_tag_map` has no entry for
> `symbol`, return the empty string. Otherwise return "<" + end_tag_map[symbol] +
> ">" (an XML-style opening tag).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.stringify-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.stringify-fn]
> `stringify(const DoubleTape &str)`. Renders a double tape to the output string,
> handling pmatch entry/exit and end-tag markers. Init retval = "", a
> stack<unsigned int> start_tag_pos, and bool input_contained_printable_symbol =
> false. Iterate each cell `it` of str: (1) if input_contained_printable_symbol
> is still false and is_printable(it->input) (SymbolNumber overload), set it true.
> (2) output = it->output. Then dispatch on output: (a) if output ==
> special_symbols[entry], push current retval.size() (as uint) onto start_tag_pos.
> (b) else if output == special_symbols[exit], pop start_tag_pos if non-empty.
> (c) else if is_end_tag(output): if container->count_patterns &&
> input_contained_printable_symbol, increment container->pattern_counts for key
> start_tag(output) (init to 1 if absent). Determine pos: if start_tag_pos is
> empty print "pmatch: warning: end tag without start tag" to stderr and use pos
> = 0, else pos = start_tag_pos.top(). If container->delete_patterns, replace the
> substring of retval from pos to end (length retval.size()-pos) with
> start_tag(output) (deleting the matched body and inserting only the start tag).
> Else if container->mark_patterns && input_contained_printable_symbol, insert
> start_tag(output) at pos and append end_tag(output) to retval (wrapping the
> body in tags). (d) else (ordinary symbol): if (!container->extract_patterns ||
> start_tag_pos is non-empty) AND is_printable(output), append
> string_from_symbol(output) to retval. Return retval.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container]
> class PmatchContainer {
>   PmatchAlphabet alphabet;
>   Encoder * encoder;
>   SymbolNumber orig_symbol_count;
>   SymbolNumber symbol_count;
>   PmatchTransducer * toplevel;
>   SymbolNumberVector input;
>   std::vector<unsigned int> entry_stack;
>   RtnCallStacks rtn_stacks;
>   hfst_ol::Transducer* uncompose_left;
>   hfst_ol::Transducer* uncompose_right;
>   DoubleTape tape;
>   DoubleTape best_result;
>   DoubleTape result;
>   LocationVectorVector locations;
>   WeightedDoubleTapeVector tape_locations;
>   std::vector<Capture> captures;
>   std::vector<Capture> best_captures;
>   std::vector<Capture> old_captures;
>   std::vector<bool> possible_first_symbols;
>   hfst::FdState<SymbolNumber> global_flag_state;
>   bool verbose;
>   bool count_patterns;
>   bool delete_patterns;
>   bool extract_patterns;
>   bool locate_mode;
>   bool mark_patterns;
>   size_t max_context_length;
>   size_t max_recursion;
>   bool need_separators;
>   bool xerox_composition;
>   bool uncomposable;
>   unsigned long line_number;
>   std::map<std::string, size_t> pattern_counts;
>   bool profile_mode;
>   bool single_codepoint_tokenization;
>   unsigned int recursion_depth_left;
>   double max_time;
>   clock_t start_clock;
>   unsigned long call_counter;
>   bool limit_reached;
>   Weight max_weight;
>   Weight running_weight;
>   Weight weight_limit;
>   unsigned int stack_depth;
>   unsigned int best_input_pos;
>   Weight best_weight;
> }

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.add-rtn-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.add-rtn-fn]
> `add_rtn(Transducer *rtn, const std::string &name)`. Copies `rtn`'s weighted
> transition table (rtn->copy_transitionw_table()) and weighted index table
> (rtn->copy_windex_table()), and constructs a new PmatchTransducer from their
> get_vector() vectors, sharing `alphabet`, with the given `name` and `this`
> container. If alphabet.has_rtn(name) is false, register it via
> alphabet.add_rtn(pmatch_rtn, name). Otherwise (the rtn name already exists)
> the freshly built pmatch_rtn is left as-is (a leak in the source) and the
> input `rtn` is deleted. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.candidate-found-fn]
> bool candidate_found(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.candidate-found-fn]
> `candidate_found()`. If `locate_mode` is true, return tape_locations.size() !=
> 0; otherwise return best_result.size() != 0. I.e. true iff a best match has
> been recorded (in the appropriate buffer for the current mode).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.collect-first-symbols-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.collect-first-symbols-fn]
> `collect_first_symbols(const std::string &symbols_list)`. Marks which symbols
> can legally begin a match. Tokenize symbols_list via
> symbol_vector_from_symbols(symbols_list) into first_symbols. For each symbol
> `*it`: while *it >= possible_first_symbols.size(), push false onto
> possible_first_symbols (growing it); then set possible_first_symbols[*it] =
> true. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.copy-to-result-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.copy-to-result-fn]
> `copy_to_result(const DoubleTape &best_result)`. Appends every cell of
> best_result, in order, onto the member `result` DoubleTape (push_back each).
> No return value. (There is also an overload copy_to_result(SymbolNumber input,
> SymbolNumber output) that pushes a single SymbolPair(input, output) onto
> result.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.decrease-stack-depth-fn]
> void decrease_stack_depth(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.decrease-stack-depth-fn]
> `decrease_stack_depth()`. If `stack_depth` == 0, throw HfstException with
> message "pmatch: negative stack depth" (HFST_THROW_MESSAGE). Otherwise
> decrement stack_depth by one. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-latest-rtn-caller-fn]
> PmatchTransducer *

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-latest-rtn-caller-fn]
> `get_latest_rtn_caller()`. Returns rtn_stacks[stack_depth - 1].back().caller —
> the `caller` PmatchTransducer* of the top (last-pushed) RtnStackFrame in the
> call stack one level below the current stack_depth. No bounds checking.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-longest-matching-capture-fn]
> std::pair<SymbolNumberVector::iterator, SymbolNumberVector::iterator>

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-longest-matching-capture-fn]
> `get_longest_matching_capture(SymbolNumber key, unsigned int input_pos)`.
> Searches recorded captures for the longest one whose name equals `key` and
> whose stored span matches the current input at `input_pos`, returning it as a
> pair of input-vector iterators [begin,end). Initialize longest_so_far =
> (input.begin(), input.begin()) (empty span). Iterate `captures`, then iterate
> `old_captures`; for each Capture `it` where key == it->name AND
> input_matches_at(input_pos, input.begin()+it->begin, input.begin()+it->end) is
> true: if its length (it->end - it->begin) is <= the current longest span
> length (longest_so_far.second - longest_so_far.first), skip it (continue);
> otherwise update longest_so_far.first = input.begin()+it->begin and
> longest_so_far.second = input.begin()+it->end. Return longest_so_far (an empty
> span if nothing matched). Note ties keep the earlier-found longer one (strict
> `>` improvement only).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-pattern-count-info-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-pattern-count-info-fn]
> `get_pattern_count_info()`. Formats `pattern_counts` (map<string,size_t>) into
> a human-readable table string. Start retval = "Pattern\t\t# of
> matches\n------------------------\n" and total = 0. For each (name, count) in
> pattern_counts (map iteration order, i.e. ascending by key), append name, then
> "\t\t", then the decimal of count (via ostringstream), then "\n"; add count to
> total. Then append "------------------------\n", "Total:\t\t", the decimal of
> total, and "\n". Return retval.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-profiling-info-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-profiling-info-fn]
> `get_profiling_info()`. Builds a profiling report string from the counter
> traversal counts. Init a stringstream retval and max_name_len = 0. Write
> "Profiling information:\n" then "  Traversals of Counter() positions:\n". Loop
> i from 0 to alphabet.counters.size()-1: if alphabet.counters[i] != NO_COUNTER,
> get counter_name = alphabet.get_counter_name(i), update max_name_len to the max
> of itself and counter_name.size(), and push (counter_name, alphabet.counters[i])
> onto counter_name_val_pairs. std::sort the pairs with counter_comp (descending
> by count). For each pair: write "    " then the name, then pad with spaces so
> the count column lines up — emit (max_name_len + 8 - name.size()) spaces — then
> the count and "\n". Return retval.str().

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-stack-depth-fn]
> unsigned int get_stack_depth(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-stack-depth-fn]
> `get_stack_depth()`. Trivial getter: returns the `stack_depth` member.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-unsatisfied-rtn-name-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-unsatisfied-rtn-name-fn]
> `get_unsatisfied_rtn_name() const`. Stub: unconditionally returns the empty
> string "".

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-weight-fn]
> Weight get_weight(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-weight-fn]
> `get_weight()`. Trivial getter: returns the `running_weight` member.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.grab-location-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.grab-location-fn]
> `grab_location(unsigned int input_pos, unsigned int tape_pos)`. Records a
> located match candidate (locate mode). If tape_locations is non-empty: if
> input_pos < best_input_pos, return immediately (existing matches are longer/
> better); else if input_pos > best_input_pos, clear best_captures and
> tape_locations (the previously recorded ones are now worse). Then set
> best_input_pos = input_pos, best_captures = captures, build a
> WeightedDoubleTape rv from tape.extract_slice(0, tape_pos) with weight
> running_weight, and push rv onto tape_locations. (So matches of equal
> input_pos accumulate; longer ones reset.) No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.has-queued-input-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.has-queued-input-fn]
> `has_queued_input(unsigned int input_pos)`. Returns input_pos < input.size()
> && (input_pos + 1 != 0). The second clause guards against unsigned underflow
> wrap-around (input_pos == UINT_MAX, which can arise from left-context checking)
> by rejecting it. True iff there is still input to read at `input_pos`.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.has-unsatisfied-rtns-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.has-unsatisfied-rtns-fn]
> `has_unsatisfied_rtns() const`. Stub: unconditionally returns false.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.increase-stack-depth-fn]
> void increase_stack_depth(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.increase-stack-depth-fn]
> `increase_stack_depth()`. Increments the `stack_depth` member by one (++).
> No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.increment-weight-fn]
> void increment_weight(Weight w)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.increment-weight-fn]
> `increment_weight(Weight w)`. Adds `w` to the `running_weight` member
> (running_weight += w). No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
> `initialize_input(const char *input_s)`. Tokenizes the C-string into the
> member `input` SymbolNumberVector, adding boundary markers and creating new
> symbols for unknown input. Clear `input`. Set up a moving char** cursor
> over the (const-cast) string. Let boundary_sym = alphabet.get_special(boundary);
> if it is not NO_SYMBOL_NUMBER, push it onto input (leading boundary). Loop
> while the cursor's current char != 0: remember original_input_loc. If
> single_codepoint_tokenization is set, compute bytes_to_tokenize =
> nByte_grapheme(cursor); if >0, malloc a scratch buffer of that many bytes +1,
> memcpy the grapheme, NUL-terminate, and call encoder->find_key(&scratch) to get
> k; if k != NO_SYMBOL_NUMBER advance the cursor by bytes_to_tokenize. Otherwise
> (normal mode) k = encoder->find_key(cursor) (which advances the cursor itself).
> If k == NO_SYMBOL_NUMBER (tokenization failed): reset cursor to
> original_input_loc, compute bytes_to_tokenize = nByte_grapheme(cursor) (if 0,
> use 1 — grab a single byte), malloc new_symbol of that size+1, memcpy and
> NUL-terminate, advance cursor by bytes_to_tokenize, then alphabet.add_symbol
> (new_symbol), encoder->read_input_symbol(new_symbol, symbol_count), set k =
> symbol_count and increment symbol_count. Push k onto input. After the loop, if
> boundary_sym != NO_SYMBOL_NUMBER push it again (trailing boundary). No return
> value. (Malloc'd buffers are not freed — leaks in the source.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.input-matches-at-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.input-matches-at-fn]
> `input_matches_at(unsigned int pos, SymbolNumberVector::iterator begin,
> SymbolNumberVector::iterator end)`. Checks whether the symbol sequence [begin,
> end) occurs in the member `input` starting at position `pos`. First, if pos +
> (end - begin) >= input.size(), return false (not enough room — note this is a
> strict >=, so a match ending exactly at the last index is also rejected).
> Otherwise for i = 0 while begin+i != end: if input[pos + i] != *(begin + i)
> return false. If all compared, return true.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.is-in-locate-mode-fn]
> bool is_in_locate_mode(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.is-in-locate-mode-fn]
> `is_in_locate_mode()`. Trivial getter: returns the `locate_mode` member.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.locate-fn]
> LocationVectorVector

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.locate-fn]
> `locate(const std::string &input, double time_cutoff, Weight weight_cutoff)`.
> Runs pmatch in locate mode and returns the per-position locations. If verbose,
> print "locating <input>" to stderr. Set max_time = time_cutoff, max_weight =
> weight_cutoff; if max_time > 0.0, set start_clock = clock(), call_counter = 0,
> limit_reached = false. Set locate_mode = true, call process(input), and return
> the member `locations` (a LocationVectorVector).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.match-fn]
> std::string

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.match-fn]
> `match(const std::string &input, double time_cutoff, Weight weight_cutoff)`.
> Runs pmatch in (non-locate) match mode and returns the stringified output. Set
> max_time = time_cutoff, max_weight = weight_cutoff; if max_time > 0.0, set
> start_clock = clock(), call_counter = 0, limit_reached = false. Set locate_mode
> = false, call process(input), and return alphabet.stringify(result) — the
> rendered result tape.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.not-possible-first-symbol-fn]
> bool not_possible_first_symbol(SymbolNumber sym)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.not-possible-first-symbol-fn]
> `not_possible_first_symbol(SymbolNumber sym)`. If possible_first_symbols is
> empty (no first-symbol filter was collected), return false (every symbol is
> allowed). Otherwise return true iff sym >= possible_first_symbols.size() OR
> possible_first_symbols[sym] == false — i.e. `sym` cannot start a match.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.note-analysis-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.note-analysis-fn]
> `note_analysis(unsigned int input_pos, unsigned int tape_pos)`. Records the
> current tape as the best match if it improves on the stored best (match mode).
> If input_pos > best_input_pos, OR (input_pos == best_input_pos AND best_weight
> > running_weight): set best_result = tape.extract_slice(0, tape_pos),
> best_captures = captures, best_input_pos = input_pos, best_weight =
> running_weight. Else if verbose AND input_pos == best_input_pos AND best_weight
> == running_weight (an equally-good conflicting match): extract the discarded
> slice tape.extract_slice(0, tape_pos) and print to stderr a message naming the
> line_number, "conflicting equally weighted matches found, keeping:" followed by
> alphabet.stringify(best_result) and "discarding:" followed by
> alphabet.stringify(discarded). No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.parse-hfst3-header-fn]
> std::map<std::string, std::string>

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.parse-hfst3-header-fn]
> `parse_hfst3_header(std::istream &f)` (static). Parses the HFST3 archive header
> from stream `f` into a map<string,string> of properties. The magic is the
> literal "HFST" followed by a NUL — i.e. 5 bytes ("HFST\0"). Loop header_loc
> from 0 to strlen("HFST")=4 inclusive (5 iterations): read one byte c = f.get();
> if c != header1[header_loc] break early. If header_loc reached 5 (full magic
> matched): read an unsigned short remaining_header_len (raw sizeof bytes); then
> f.get() must be '\0' else throw TransducerHeaderException. new[] a char buffer
> headervalue of remaining_header_len bytes and f.read it; if its last byte is
> not '\0', throw TransducerHeaderException. Walk the buffer with index i from 0:
> read a NUL-terminated `property` string (length via strlen at offset i),
> advance i past it+1, read a NUL-terminated `value` similarly, set
> properties[property] = value, advance i past it+1; repeat while i <
> remaining_header_len. delete[] the buffer and return properties. If the magic
> did NOT fully match (else branch): unget the non-matching character, then unget
> each of the header_loc characters that did match (loop i from header_loc-1 down
> to 0), and throw TransducerHeaderException.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
> PmatchContainer::PmatchContainer(std::vector<HfstTransducer> transducers)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
> Constructor `PmatchContainer(std::vector<HfstTransducer> transducers)`.
> Member init: entry_stack empty, verbose=false, locate_mode=false,
> line_number=0, profile_mode=false, single_codepoint_tokenization=false,
> running_weight=0.0. Body: call set_properties() (the no-arg defaults
> overload) then reset_recursion(). If transducers is empty, return. Take
> properties = transducers[0].get_properties() and call
> set_properties(properties) (the map overload).
> Single-transducer case (size==1): pick `top`; if transducers[0] is not
> HFST_OLW_TYPE, copy it to a new HfstTransducer and convert to HFST_OLW_TYPE,
> else use &transducers[0]. Convert to a backend hfst_ol::Transducer via
> ConversionFunctions::hfst_transducer_to_hfst_ol(top), read its
> TransducerHeader, set alphabet = PmatchAlphabet(backend->get_alphabet(),
> this), orig_symbol_count = symbol_count =
> alphabet.get_orig_symbol_count(), global_flag_state = alphabet.get_fd_table(),
> encoder = new Encoder(alphabet.get_symbol_table(), orig_symbol_count). Copy
> the weighted transition/index tables and build toplevel = new
> PmatchTransducer(transitions.get_vector(), indices.get_vector(), alphabet,
> "TOP", this). If a temp `top` was allocated, delete it.
> Multi-transducer case (size>1): harmonize all transducers to a common
> alphabet. Build a dummy TROPICAL_OPENFST_TYPE `harmonizer`: for each
> transducer iterate its alphabet StringSet and disjunct each not-yet-seen
> symbol into harmonizer (tracking symbols_seen). While iterating, classify
> each transducer: the one named "TOP" becomes `top` (converting to OLW if
> needed), all others go into temporaries[i] (converting to OLW if needed). If
> no TOP found, print "pmatch: warning: TOP not defined in archive, using first
> as TOP" to stderr and use temporaries[0] as top. Convert harmonizer to
> HFST_OLW_TYPE. For TOP: convert via hfst_transducer_to_hfst_basic_transducer
> then hfst_basic_transducer_to_hfst_ol(intermediate, weighted=true, "",
> &harmonizer); set alphabet/orig_symbol_count/symbol_count/global_flag_state/
> encoder and build toplevel exactly as in the single case. Then for each
> non-NULL temporaries[i]: harmonize the same way, copy its weighted tables,
> build a PmatchTransducer rtn named temporaries[i]->get_name() and call
> alphabet.add_rtn(rtn, name). Finally delete any temporaries that were
> freshly allocated (those whose original type wasn't HFST_OLW_TYPE), handling
> TOP and the others separately. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.process-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.process-fn]
> `process(const std::string &input_str)`. The main matching driver. If verbose,
> print "PC::processing <input_str>" to stderr. Call
> initialize_input(input_str.c_str()). Init input_pos=0, printable_input_pos=0,
> running_weight=0.0, stack_depth=0, best_input_pos=0; ++line_number; clear
> result, locations, old_captures, best_captures, captures; reset_recursion();
> declare a DoubleTape nonmatching_locations. Main loop while
> has_queued_input(input_pos): clear best_result; current_input = input[input_pos].
> (A) If not_possible_first_symbol(current_input): copy_to_result(current_input,
> current_input), ++input_pos, and if locate_mode && is_printable(current_input)
> then ++printable_input_pos and push SymbolPair(current_input,current_input)
> onto nonmatching_locations; then continue. (B) Otherwise clear tape and
> tape_locations, tape_pos=0, old_input_pos=input_pos, call
> toplevel->match(input_pos, tape_pos). If candidate_found(): in locate_mode,
> first flush any accumulated nonmatching_locations as a single Location via
> locatefy(printable_input_pos - nonmatching_locations.size(),
> WeightedDoubleTape(nonmatching_locations, 0.0)) with output set to
> "@_NONMATCHING_@", pushed as a one-element LocationVector onto locations, then
> clear nonmatching_locations; then for each WeightedDoubleTape in tape_locations
> build a Location via locatefy(printable_input_pos, *it), collect into a
> LocationVector ls, sort(ls) (ascending by weight), push ls onto locations, and
> add (best_input_pos - old_input_pos) to printable_input_pos. In non-locate
> mode instead copy_to_result(best_result). Either way set input_pos =
> best_input_pos and append best_captures onto old_captures. (C) If
> !candidate_found() OR input_pos == old_input_pos (no input consumed): if
> verbose print "no candidate found"; copy_to_result(current_input,
> current_input); ++input_pos; and if locate_mode && is_printable(current_input)
> then ++printable_input_pos and push the symbol onto nonmatching_locations. After
> the loop: if locate_mode and nonmatching_locations is non-empty, flush them as a
> final "@_NONMATCHING_@" Location onto locations the same way. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.push-rtn-call-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.push-rtn-call-fn]
> `push_rtn_call(unsigned int return_index, PmatchTransducer *caller)`. Builds an
> RtnStackFrame new_top with caller = `caller` and caller_index = `return_index`.
> If rtn_stacks.size() <= stack_depth (no call stack exists yet at this depth),
> push a new RtnCallStack containing just new_top (RtnCallStack(1, new_top)) onto
> rtn_stacks; otherwise push new_top onto rtn_stacks[stack_depth]. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.reset-recursion-fn]
> void reset_recursion(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.reset-recursion-fn]
> `reset_recursion()`. Sets recursion_depth_left = (unsigned int)max_recursion,
> restoring the remaining-recursion budget to its configured maximum. No return
> value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.rtn-stack-pop-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.rtn-stack-pop-fn]
> `rtn_stack_pop()`. Calls pop_back() on rtn_stacks[stack_depth], removing the
> top (last-pushed) RtnStackFrame of the call stack at the current depth. No
> bounds checking. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.rtn-stack-top-fn]
> RtnStackFrame

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.rtn-stack-top-fn]
> `rtn_stack_top()`. Returns rtn_stacks[stack_depth].back() — a copy of the top
> (last-pushed) RtnStackFrame in the call stack at the current stack_depth. No
> bounds checking.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-count-patterns-fn]
> void set_count_patterns(bool b)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-count-patterns-fn]
> `set_count_patterns(bool b)`. Trivial setter: assigns count_patterns = b. No
> return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-delete-patterns-fn]
> void set_delete_patterns(bool b)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-delete-patterns-fn]
> `set_delete_patterns(bool b)`. Trivial setter: assigns delete_patterns = b. No
> return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-extract-patterns-fn]
> void set_extract_patterns(bool b)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-extract-patterns-fn]
> `set_extract_patterns(bool b)`. Trivial setter: assigns extract_patterns = b.
> No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-locate-mode-fn]
> void set_locate_mode(bool b)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-locate-mode-fn]
> `set_locate_mode(bool b)`. Trivial setter: assigns locate_mode = b. No return
> value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-mark-patterns-fn]
> void set_mark_patterns(bool b)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-mark-patterns-fn]
> `set_mark_patterns(bool b)`. Trivial setter: assigns mark_patterns = b. No
> return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-max-context-fn]
> void set_max_context(size_t max)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-max-context-fn]
> `set_max_context(size_t max)`. Trivial setter: assigns max_context_length =
> max. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-max-recursion-fn]
> void set_max_recursion(size_t max)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-max-recursion-fn]
> `set_max_recursion(size_t max)`. Trivial setter: assigns max_recursion = max.
> No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-profile-fn]
> void set_profile(bool b)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-profile-fn]
> `set_profile(bool b)`. Trivial setter: assigns profile_mode = b. No return
> value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-properties-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-properties-fn]
> Two overloads. (1) `set_properties(void)` installs defaults: count_patterns
> =false, delete_patterns=false, extract_patterns=false, locate_mode=false,
> mark_patterns=true, max_context_length=254, max_recursion=5000,
> need_separators=true, xerox_composition=true, uncomposable=false. No return
> value. (2) `set_properties(std::map<std::string,std::string> &properties)`
> iterates the map and for each recognized key applies its value: keys
> "count-patterns", "delete-patterns", "extract-patterns", "mark-patterns",
> "need-separators" map "on"->true / "off"->false on the corresponding bool
> member (other values ignored); "locate-patterns" sets locate_mode on/off the
> same way; "xerox-composition" sets xerox_composition (note: checks "off" first
> then "on"). For "max-context-length": parse the value into max_context_length
> via stringstream; if the parsed result is 0 but the string was not literally
> "0", reset max_context_length = 254 (treat unparseable as the default). For
> "max-recursion": parse into max_recursion the same way; if 0 but not literal
> "0", reset to 5000. Unknown keys are ignored. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-single-codepoint-tokenization-fn]
> void set_single_codepoint_tokenization(bool b)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-single-codepoint-tokenization-fn]
> `set_single_codepoint_tokenization(bool b)`. Trivial setter: assigns
> single_codepoint_tokenization = b. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-verbose-fn]
> void set_verbose(bool b)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-verbose-fn]
> `set_verbose(bool b)`. Trivial setter: assigns verbose = b. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-weight-fn]
> void set_weight(Weight w)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-weight-fn]
> `set_weight(Weight w)`. Trivial setter: assigns running_weight = w. No return
> value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.symbol-vector-from-symbols-fn]
> SymbolNumberVector

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.symbol-vector-from-symbols-fn]
> `symbol_vector_from_symbols(const std::string &symbols)`. Tokenizes `symbols`
> into a SymbolNumberVector by reusing initialize_input(symbols.c_str()) (which
> populates the member `input`, adding boundary markers). If
> alphabet.get_special(boundary) != NO_SYMBOL_NUMBER (boundaries were added),
> return a copy of input excluding the leading and trailing boundary, i.e.
> SymbolNumberVector(input.begin()+1, input.end()-1). Otherwise return a copy of
> the whole input. Note: this mutates the member `input` as a side effect.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.try-recurse-fn]
> bool try_recurse(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.try-recurse-fn]
> `try_recurse()`. If recursion_depth_left > 0, decrement it (--) and return
> true (recursion permitted, budget consumed). Otherwise return false (budget
> exhausted). No other side effects.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.uncompose-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.uncompose-fn]
> `uncompose(Location &loc)`. Attempts to recover the intermediate ("middle")
> form of a composed match by running the two stored uncompose transducers. If
> `uncomposable` is false, optionally print "uncompose disabled" (verbose) and
> return immediately. Optionally print "uncomposing left <loc.input>". Compute
> middle_left = uncompose_left->lookup_fd(loc.input); if it is empty (ambiguity/
> failure) optionally print "empty midleft compose" and return. Build a set
> midforms. For each lpath in *middle_left: concatenate its output symbols
> (skipping flag diacritics via FdOperation::is_diacritic) into a string `mids`;
> compute middle_right = uncompose_right->lookup_fd(mids); if empty, continue.
> For each rpath in *middle_right: concatenate its non-diacritic output symbols
> into `lows`; if lows == loc.output, insert mids into midforms (a successful
> round-trip); otherwise (verbose) note no match. After processing, if
> midforms.size() > 1 nothing special is done (an ambiguity comment only). For
> each form in midforms set loc.middle = form (so loc.middle ends up holding the
> last form in set iteration order). Mutates `loc.middle`; verbose prints to
> stderr throughout. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-container.unrecurse-fn]
> void unrecurse(void)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.unrecurse-fn]
> `unrecurse()`. Increments recursion_depth_left by one (++), restoring one unit
> of recursion budget when returning from a recursive call. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer]
> class PmatchTransducer {
>   std::string name;
>   enum ContextChecking{none, LC, NLC, RC, NRC};
>   struct LocalVariables { hfst::FdState<SymbolNumber> flag_state; // Used for context checks char tape_step; size_t max_context_length_remaining; unsigned int ...;
>   std::stack<LocalVariables> local_stack;
>   std::vector<TransitionW> transition_table;
>   std::vector<TransitionWIndex> index_table;
>   PmatchAlphabet & alphabet;
>   SymbolNumber orig_symbol_count;
>   PmatchContainer * container;
> }

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.check-context-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.check-context-fn]
> `check_context(unsigned int input_pos, unsigned int tape_pos,
> TransitionTableIndex i)`. Performs a context check after a context-entry arc.
> Record the current input_pos into local_stack.top().context_placeholder
> (remembers where to resume if the check succeeds). If the current context is LC
> or NLC (left context), jump the input position to the left-hand side of input:
> input_pos = container->entry_stack.back() - 1. Recurse into
> get_analyses(input_pos, tape_pos, transition_table[i].get_target()) to walk the
> context sub-automaton. After it returns, compute schedule_passthrough: if the
> context is NLC or NRC (negative) AND
> local_stack.top().negative_context_success == false (the negative context did
> NOT match, which is the success condition for a negation), set it true. Pop the
> local_stack frame that was pushed when entering the context. If
> schedule_passthrough, set the now-current local_stack.top().pending_passthrough
> = true (so a passthrough arc is taken later). No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.checking-context-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.checking-context-fn]
> `checking_context() const`. Returns local_stack.top().context != none — i.e.
> true iff the top local-variables frame is currently inside a context check
> (LC/RC/NLC/NRC), false when context == none.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.context-checking]
> enum ContextChecking {
>   none;
>   LC;
>   NLC;
>   RC;
>   NRC;
> }

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.exit-context-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.exit-context-fn]
> `exit_context()`. Pushes a fresh local-variables frame that ends the current
> context check. Copy the current local_stack.top() into new_top, then set
> new_top.context = none, new_top.negative_context_success = false, and
> new_top.tape_step = 1 (forward stepping resumed). Push new_top onto
> local_stack. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.final-index-fn]
> bool final_index(TransitionTableIndex i) const

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.final-index-fn]
> `final_index(TransitionTableIndex i) const`. If indexes_transition_table(i)
> (i.e. i >= TRANSITION_TARGET_TABLE_START), return transition_table[i].final();
> otherwise return index_table[i].final(). Note: in the transition-table branch
> the index is used directly (NOT offset by TRANSITION_TARGET_TABLE_START),
> matching the source. Returns whether the addressed cell is final.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.get-analyses-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.get-analyses-fn]
> `get_analyses(unsigned int input_pos, unsigned int tape_pos,
> TransitionTableIndex i)`. The central recursive traversal step. Early returns:
> (1) if container->get_weight() > container->max_weight, return. (2) If
> container->max_time > 0.0: ++container->call_counter; if limit_reached is
> already set, OR (call_counter % 1000000 == 0 AND a candidate has been found AND
> elapsed wall time (clock()-start_clock)/CLOCKS_PER_SEC > max_time), set
> limit_reached = true and return. (3) If !container->try_recurse() (recursion
> budget exhausted), optionally print "pmatch: out of stack space, truncating
> result" (verbose) and return. Then set local_stack.top().default_symbol_trap =
> true and call take_epsilons(input_pos, tape_pos, i + 1). If
> local_stack.top().pending_passthrough is true, clear it and call
> take_transitions(get_special(Pmatch_passthrough), input_pos, tape_pos, i + 1)
> (a negative context that failed-successfully). If is_final(i): save old_weight,
> increment_weight(get_weight(i)), call handle_final_state(input_pos, tape_pos),
> then restore set_weight(old_weight). Next, if !has_queued_input(input_pos):
> container->unrecurse() and return; else input = container->input[input_pos].
> Then take transitions for each applicable symbol class, all into i+1: if
> symbol2lists[input] != NO_SYMBOL_NUMBER, for each symbol in
> symbol_lists[symbol2lists[input]] call take_transitions(that, ...). For each of
> the four Unicode specials (UnicodeAlpha, UnicodeUpperAlpha, UnicodeLowerAlpha,
> UnicodeWhitespace) whose special symbol is defined AND whose predicate
> (is_unicode_alpha / upperalpha / loweralpha / whitespace) holds for input, call
> take_transitions(that special, ...). For the literal input: if input <
> orig_symbol_count, take_transitions(input, ...); else take identity and unknown
> specials if defined (get_identity_symbol / get_unknown_symbol). Finally if the
> default symbol is defined AND local_stack.top().default_symbol_trap is set, take
> the default symbol. Then container->unrecurse() and return. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.get-weight-fn]
> Weight get_weight(TransitionTableIndex i)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.get-weight-fn]
> `get_weight(TransitionTableIndex i)`. If indexes_transition_table(i) (i.e. i >=
> TRANSITION_TARGET_TABLE_START), return transition_table[i -
> TRANSITION_TARGET_TABLE_START].get_weight() (the arc weight); otherwise return
> index_table[i].final_weight() (the final weight at an index-table state).

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.handle-final-state-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.handle-final-state-fn]
> `handle_final_state(unsigned int input_pos, unsigned int tape_pos)`. Dispatches
> on the container's current state. If container->get_stack_depth() > 0 (we are
> inside an rtn call, not toplevel): get rtn_target =
> container->get_latest_rtn_caller() and call rtn_target->rtn_return(input_pos,
> tape_pos) to return control to the caller. Else if
> container->is_in_locate_mode(): call container->grab_location(input_pos,
> tape_pos). Else: call container->note_analysis(input_pos, tape_pos). No return
> value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.indexes-transition-table-fn]
> static bool indexes_transition_table(TransitionTableIndex i)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.indexes-transition-table-fn]
> `indexes_transition_table(TransitionTableIndex i)` (static). Returns i >=
> TRANSITION_TARGET_TABLE_START — true iff the index addresses the transition
> table rather than the index table.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.is-final-fn]
> bool is_final(TransitionTableIndex i)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.is-final-fn]
> `is_final(TransitionTableIndex i)`. If indexes_transition_table(i) (i.e. i >=
> TRANSITION_TARGET_TABLE_START), return
> transition_table[i - TRANSITION_TARGET_TABLE_START].final(); otherwise return
> index_table[i].final(). Reports whether the state at index `i` is a final
> state in the appropriate table.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.is-good-fn]
> static bool is_good(TransitionTableIndex i)

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.is-good-fn]
> `is_good(TransitionTableIndex i)` (static). Returns i <
> TRANSITION_TARGET_TABLE_START — true iff `i` is a valid in-range index into
> the transition table (the inverse of indexes_transition_table); used as the
> loop-continuation test when walking transitions.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.local-variables]
> struct LocalVariables {
>   hfst::FdState<SymbolNumber> flag_state;
>   char tape_step;
>   size_t max_context_length_remaining;
>   unsigned int context_placeholder;
>   ContextChecking context;
>   bool default_symbol_trap;
>   bool negative_context_success;
>   bool pending_passthrough;
> }

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.make-transition-table-index-fn]
> TransitionTableIndex make_transition_table_index(

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.make-transition-table-index-fn]
> `make_transition_table_index(TransitionTableIndex i, SymbolNumber input)`.
> Resolves an index/transition-table index into a transition-table offset for
> the given `input` symbol. If indexes_transition_table(i) (i already points
> into the transition table), return i - TRANSITION_TARGET_TABLE_START.
> Otherwise (i points into the index table): look at index_table[i + input]; if
> its input symbol equals `input`, return its target minus
> TRANSITION_TARGET_TABLE_START (the resolved transition-table offset);
> otherwise return TRANSITION_TARGET_TABLE_START (which is NOT is_good, so a
> caller's `while (is_good(...))` loop will not execute — meaning "no matching
> transition").

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.match-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.match-fn]
> `match(unsigned int input_tape_pos, unsigned int tape_pos)`. Entry point for
> matching at a given input position. Resets the top LocalVariables frame:
> context = none, tape_step = 1, context_placeholder = 0, default_symbol_trap =
> false. Then calls get_analyses(input_tape_pos, tape_pos, 0) (starting at
> index 0, the transducer's start state) to perform the recursive traversal. No
> return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.pmatch-transducer-fn]
> PmatchTransducer::PmatchTransducer(std::istream &is,

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.pmatch-transducer-fn]
> Constructor `PmatchTransducer(std::istream &is, TransitionTableIndex
> index_table_size, TransitionTableIndex transition_table_size, PmatchAlphabet
> &alpha, std::string _name, PmatchContainer *cont)`. Member init list: name =
> _name, alphabet = alpha (reference), container = cont. Body: set
> orig_symbol_count = size_t_to_uint(alphabet.get_symbol_table().size()).
> Initialize the local-variable stack: build a LocalVariables with flag_state =
> alphabet.get_fd_table(), tape_step = 1, max_context_length_remaining = 254,
> context = none, context_placeholder = 0, default_symbol_trap = false,
> negative_context_success = false, pending_passthrough = false, and push it
> onto local_stack. Then read the tables from `is`: malloc indextab of
> TransitionWIndex::size * index_table_size bytes and transitiontab of
> TransitionW::size * transition_table_size bytes, is.read both; reserve
> index_table and, while index_table_size > 0, push_back TransitionWIndex(cursor)
> advancing the cursor by TransitionWIndex::size and decrementing the count;
> free the index buffer; do the same for transition_table with TransitionW and
> TransitionW::size; free the transition buffer. No return value. (A second
> constructor overload instead takes ready-made transition_vector and
> index_vector by value, moving them into transition_table/index_table, and does
> the same orig_symbol_count computation and local_stack initialization but no
> table reading.)

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-call-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-call-fn]
> `rtn_call(unsigned int input_tape_pos, unsigned int tape_pos, PmatchTransducer
> *caller, TransitionTableIndex caller_index)`. Invokes this RTN (subroutine
> transducer) from a calling transducer. Steps: container->push_rtn_call(
> caller_index, caller) (records the return target on the container's rtn
> stack); container->increase_stack_depth(); build new_top as a copy of
> local_stack.top() then override flag_state = alphabet.get_fd_table(),
> tape_step = 1, context = none, context_placeholder = 0, default_symbol_trap =
> false; push new_top onto local_stack; call get_analyses(input_tape_pos,
> tape_pos, 0) (traverse this RTN from its start state); then local_stack.pop(),
> container->decrease_stack_depth(), container->rtn_stack_pop(). No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-call-in-context-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-call-in-context-fn]
> `rtn_call_in_context(unsigned int input_tape_pos, unsigned int tape_pos,
> PmatchTransducer *caller, TransitionTableIndex caller_index, LocalVariables
> locals)`. Like rtn_call but used while a context check is in progress, so it
> preserves the supplied `locals` (the context state) instead of resetting it.
> Steps: container->push_rtn_call(caller_index, caller);
> container->increase_stack_depth(); build new_top as a copy of `locals` and
> override only new_top.flag_state = alphabet.get_fd_table() (keeping its
> context/tape_step/etc.); push new_top onto local_stack; call
> get_analyses(input_tape_pos, tape_pos, 0); then local_stack.pop(),
> container->decrease_stack_depth(), container->rtn_stack_pop(). No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-return-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-return-fn]
> `rtn_return(unsigned int input_tape_pos, unsigned int tape_pos)`. Returns from
> an RTN back into the caller's traversal. Steps:
> container->decrease_stack_depth(); read entry_index =
> container->rtn_stack_top().caller_index (the index in the caller where the
> call was made); call get_analyses(input_tape_pos, tape_pos, entry_index) on
> `this` (the caller transducer, since rtn_return is invoked on the caller),
> resuming the caller's traversal at that index; then
> container->increase_stack_depth() to restore the depth. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-epsilons-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-epsilons-fn]
> `take_epsilons(unsigned int input_pos, unsigned int tape_pos,
> TransitionTableIndex i)`. Walks epsilon/flag/RTN/special transitions out of
> state `i`. Set i = make_transition_table_index(i, 0). While is_good(i): read
> input = transition_table[i] input symbol. If input != 0 and input is NOT a
> flag diacritic and NOT an RTN symbol, return immediately (epsilon transitions
> are sorted first; once a real input symbol is seen we are done). Read output,
> target = transition_table[i] output/target, save old_weight =
> container->get_weight() and container->increment_weight by this transition's
> weight. Then branch:
> > (1) If checking_context(): if try_exiting_context(output) succeeds, the
> > context check completed — call get_analyses(local_stack.top()
> > .context_placeholder, tape_pos, target) then local_stack.pop(). Else if
> > local_stack.top().negative_context_success is true, return (a negative
> > context matched, back out). Else if input is a flag diacritic, take_flag(
> > input, input_pos, tape_pos, i). Else if input has an RTN,
> > alphabet.get_rtn(input)->rtn_call_in_context(input_pos, tape_pos, this,
> > target, local_stack.top()). Else get_analyses(input_pos, tape_pos, target)
> > (no tape change while checking context).
> > (2) Else if input == 0 (a true epsilon arc): if container->profile_mode,
> > alphabet.count(output). If !try_entering_context(output) (output is not a
> > context-entry marker): write container->tape.write(tape_pos, 0, output);
> > then adjust the entry stack by output type: if output == get_special(entry)
> > push input_pos onto container->entry_stack; else if output ==
> > get_special(exit) save orig_entry_stack_back = entry_stack.back() and
> > pop_back; else if is_capture_tag(output) push a Capture {begin =
> > entry_stack.back(), end = input_pos, name = output} onto container->captures;
> > else if is_captured_tag(output) call get_longest_matching_capture(
> > captured2capture[output], input_pos) and, if the returned span is non-empty,
> > container->tape.write(tape_pos, cap) and get_analyses(input_pos + span_len,
> > tape_pos + span_len, target), then ++i, container->set_weight(old_weight),
> > and continue (skip the common get_analyses below). For the non-captured
> > cases, call get_analyses(input_pos, tape_pos + 1, target), then UNDO the
> > entry-stack adjustment: if entry, pop_back; if exit, push back
> > orig_entry_stack_back; if capture tag, captures.pop_back(). If
> > try_entering_context(output) returned true instead (output WAS a context
> > marker, and the context frame was pushed), call check_context(input_pos,
> > tape_pos, i).
> > (3) Else if input is a flag diacritic: take_flag(input, input_pos, tape_pos,
> > i).
> > (4) Else if input has an RTN: alphabet.get_rtn(input)->rtn_call(input_pos,
> > tape_pos, this, target).
> After the branch: ++i and container->set_weight(old_weight), continue the loop.
> No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-flag-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-flag-fn]
> `take_flag(SymbolNumber input, unsigned int input_pos, unsigned int tape_pos,
> TransitionTableIndex i)`. Applies a flag-diacritic transition and recurses if
> the flag's operation is allowed by the current flag state. Steps: declare
> old_global_values. If alphabet.is_global_flag(input): save old_global_values =
> container->global_flag_state.get_values(); apply the operation via
> container->global_flag_state.apply_operation(*alphabet.get_operation(input));
> if that returns false, return immediately (global flag disallowed). Save
> old_values = local_stack.top().flag_state.get_values(). If
> local_stack.top().flag_state.apply_operation(*alphabet.get_operation(input))
> returns true (flag allowed), call get_analyses(input_pos, tape_pos,
> transition_table[i].get_target()) — flags are not written to the tape. Then
> restore state: if is_global_flag(input),
> container->global_flag_state.assign_values(old_global_values); always
> local_stack.top().flag_state.assign_values(old_values). No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-transitions-fn]
> void

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-transitions-fn]
> `take_transitions(SymbolNumber input, unsigned int input_pos, unsigned int
> tape_pos, TransitionTableIndex i)`. Walks the non-epsilon transitions out of
> state `i` that consume the symbol `input`. Set i =
> make_transition_table_index(i, input). While is_good(i): read this_input,
> this_output, target from transition_table[i]. If this_input ==
> NO_SYMBOL_NUMBER, return. Else if this_input == input: save old_weight =
> container->get_weight() and increment_weight by this transition's weight.
> > If !checking_context(): if alphabet.is_meta_arc(this_output) OR
> > alphabet.list2symbols[this_output] != NO_SYMBOL_NUMBER (we arrived via a
> > meta/list arc), overwrite this_output = this_input = container->input[
> > input_pos] (use the actual input symbol from the input tape). Then if
> > this_input == get_special(Pmatch_passthrough), call get_analyses(input_pos,
> > tape_pos, target) (consume nothing); otherwise container->tape.write(
> > tape_pos, this_input, this_output) and get_analyses(input_pos + 1, tape_pos +
> > 1, target) (consume one input and one tape cell).
> > Else (checking_context()): do not touch the output tape. If
> > local_stack.top().max_context_length_remaining > 0: if tape_step < 0 and
> > input_pos == 0 (would underflow), call get_analyses(input_pos, tape_pos,
> > target) without moving; otherwise decrement max_context_length_remaining,
> > call get_analyses(input_pos + tape_step, tape_pos, target) (move by the
> > signed tape_step, +1 for right context, -1 for left), then increment
> > max_context_length_remaining back.
> After handling: set local_stack.top().default_symbol_trap = false and
> container->set_weight(old_weight). Else (this_input != input), return. Then
> ++i and continue the loop. No return value.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.try-entering-context-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.try-entering-context-fn]
> `try_entering_context(SymbolNumber symbol)`. Detects whether `symbol` is a
> context-entry marker and, if so, pushes a new context frame. Build a fresh
> LocalVariables new_top. Branch on equality with the special symbols: if symbol
> == get_special(LC_entry): new_top = copy of local_stack.top(), context = LC,
> tape_step = -1. Else if == get_special(RC_entry): copy top, context = RC,
> tape_step = 1. Else if == get_special(NLC_entry): copy top, context = NLC,
> tape_step = -1. Else if == get_special(NRC_entry): copy top, context = NRC,
> tape_step = 1. Else return false (not a context entry). On a match, set
> new_top.max_context_length_remaining = container->max_context_length, push
> new_top onto local_stack, and return true.

> [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.try-exiting-context-fn]
> bool

> [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.try-exiting-context-fn]
> `try_exiting_context(SymbolNumber symbol)`. Tests whether `symbol` is the
> matching context-exit marker for the current context. switch on
> local_stack.top().context: case LC: if symbol == get_special(LC_exit) call
> exit_context() and return true, else return false. case RC: if symbol ==
> get_special(RC_exit) call exit_context() and return true, else return false.
> case NRC: if symbol == get_special(NRC_exit) set
> local_stack.top().negative_context_success = true and return false (note: no
> break, so on a non-match it falls through into the NLC case). case NLC: if
> symbol == get_special(NLC_exit) set negative_context_success = true and return
> false. default: return false. (For positive contexts a successful exit
> dismantles the context frame via exit_context() and returns true; for negative
> contexts reaching the exit marker means the forbidden context matched, so it
> records negative_context_success and returns false.)

> [spec:hfst:def:pmatch.hfst-ol.rtn-call-stack]
> typedef std::vector<RtnStackFrame> RtnCallStack

> [spec:hfst:def:pmatch.hfst-ol.rtn-call-stacks]
> typedef std::vector<RtnCallStack> RtnCallStacks

> [spec:hfst:def:pmatch.hfst-ol.rtn-name-map]
> typedef std::map<std::string, SymbolNumber> RtnNameMap

> [spec:hfst:def:pmatch.hfst-ol.rtn-stack-frame]
> struct RtnStackFrame {
>   PmatchTransducer * caller;
>   TransitionTableIndex caller_index;
> }

> [spec:hfst:def:pmatch.hfst-ol.rtn-vector]
> typedef std::vector<PmatchTransducer *> RtnVector

> [spec:hfst:def:pmatch.hfst-ol.special-symbol]
> enum SpecialSymbol {
>   entry;
>   exit;
>   LC_entry;
>   LC_exit;
>   RC_entry;
>   RC_exit;
>   NLC_entry;
>   NLC_exit;
>   NRC_entry;
>   NRC_exit;
>   Pmatch_passthrough;
>   boundary;
>   Pmatch_input_mark;
>   UnicodeAlpha;
>   UnicodeUpperAlpha;
>   UnicodeLowerAlpha;
>   UnicodeWhitespace;
>   SPECIALSYMBOL_NR_ITEMS;
> }

> [spec:hfst:def:pmatch.hfst-ol.weighted-double-tape-vector]
> typedef std::vector<WeightedDoubleTape> WeightedDoubleTapeVector

