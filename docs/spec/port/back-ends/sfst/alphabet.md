# back-ends/sfst/alphabet.cc, back-ends/sfst/alphabet.h

> [spec:hfst:def:alphabet.sfst.alphabet]
> class Alphabet {
>   struct eqstr { // [spec:hfst:def:alphabet.sfst.alphabet.eqstr.operator-fn] // [spec:hfst:sem:alphabet.sfst.alphabet.eqstr.operator-fn] bool operator()(const ...;
>   SymbolMap sm;
>   CharMap cm;
>   LabelSet ls;
>   bool utf8;
> }

> [spec:hfst:def:alphabet.sfst.alphabet.add-fn]
> void Alphabet::add( const char *symbol, Character c )

> [spec:hfst:sem:alphabet.sfst.alphabet.add-fn]
> Registers a symbol/code pair unconditionally (no existence or
> conflict checks). Duplicates `symbol` into a freshly heap-allocated
> C string `s` (via fst_strdup). Sets the code-to-symbol map entry
> `cm[c] = s` and the symbol-to-code map entry `sm[s] = c`, both
> keyed on the same allocated string. The allocation is owned by the
> alphabet (freed in clear()). Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.add-symbol-fn]
> void Alphabet::add_symbol( const char *symbol, Character c )

> [spec:hfst:sem:alphabet.sfst.alphabet.add-symbol-fn]
> Adds `symbol` to the alphabet bound to the explicit code `c`, with
> consistency checks. Step 1: look up the symbol's existing code via
> symbol2code(symbol). If it is not EOF (symbol already defined): if
> that existing code equals `c`, return (no-op); otherwise throw an
> error string — if strlen(symbol) < 60 format a message "Error:
> reinserting symbol '<symbol>' in alphabet with incompatible
> character value <sc> <c>" into a static buffer and throw it, else
> throw a fixed "reinserting symbol in alphabet with incompatible
> character value" string. Step 2: look up the symbol currently
> mapped to code `c` via code2symbol(c). If NULL (code unused), call
> add(symbol, c) to register the pair. Otherwise if the existing
> symbol string differs from `symbol` (strcmp != 0), throw an error:
> if strlen(symbol) < 70 format "Error: defining symbol <symbol> as
> character <c> (previously defined as <s>)" into a static buffer
> else a generic long-symbol message, and throw it. If the existing
> symbol equals `symbol`, do nothing. Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.alphabet-fn]
> Alphabet::Alphabet()

> [spec:hfst:sem:alphabet.sfst.alphabet.alphabet-fn]
> Default constructor. Sets the `utf8` flag to false. Then calls
> add(EpsilonString, Label::epsilon), i.e. registers the epsilon
> symbol — the literal string "<>" — with code 0, so every alphabet
> starts containing the epsilon mapping in both sm and cm. The label
> set ls starts empty.

> [spec:hfst:def:alphabet.sfst.alphabet.begin-fn]
> const_iterator begin() const

> [spec:hfst:sem:alphabet.sfst.alphabet.begin-fn]
> Const accessor. Returns ls.begin(), a const_iterator to the first
> label in the alphabet's label set ls (ordered by Label::label_cmp).

> [spec:hfst:def:alphabet.sfst.alphabet.char-map]
> typedef hash_map<Character, char*> CharMap

> [spec:hfst:def:alphabet.sfst.alphabet.clear-char-pairs-fn]
> void clear_char_pairs()

> [spec:hfst:sem:alphabet.sfst.alphabet.clear-char-pairs-fn]
> Clears the label set ls (ls.clear()), removing all known
> labels/character pairs. Leaves the symbol map sm and code map cm
> (and the utf8 flag) untouched. Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.clear-fn]
> void Alphabet::clear()

> [spec:hfst:sem:alphabet.sfst.alphabet.clear-fn]
> Fully empties the alphabet and frees the heap-allocated symbol
> strings. Allocates a temporary char* array of size cm.size().
> Clears the label set ls and the symbol map sm. Then iterates over
> cm collecting each entry's value pointer (it->second) into the temp
> array (count n), and clears cm. Finally frees each collected
> pointer with free() (these were allocated by fst_strdup in add())
> and deletes the temp array. After this sm, cm, ls are all empty and
> the owned C strings are released. Returns nothing. (Note: cm and sm
> share the same string pointers, so freeing once via the cm-derived
> list is correct.)

> [spec:hfst:def:alphabet.sfst.alphabet.code2symbol-fn]
> const char *code2symbol( Character c ) const

> [spec:hfst:sem:alphabet.sfst.alphabet.code2symbol-fn]
> Const lookup. Searches the code map cm for code `c`. If found,
> returns the associated symbol string (p->second). If not present,
> returns NULL.

> [spec:hfst:def:alphabet.sfst.alphabet.complement-fn]
> void Alphabet::complement( vector<Character> &sym )

> [spec:hfst:sem:alphabet.sfst.alphabet.complement-fn]
> Replaces the input symbol vector `sym` in place with its complement
> over the alphabet's known character codes. Builds a local `result`
> vector. Iterates over every entry in the code map cm; for each code
> `c` that is not Label::epsilon, performs a linear scan of `sym` for
> `c`; if `c` is not found in `sym`, appends it to `result`.
> Epsilon is always excluded. Finally swaps `result` into `sym`
> (sym now holds all non-epsilon codes of the alphabet not originally
> present in sym). Order follows cm's iteration order. Returns
> nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.compose-fn]
> void Alphabet::compose( const Alphabet &la, const Alphabet &ua )

> [spec:hfst:sem:alphabet.sfst.alphabet.compose-fn]
> Builds this alphabet as the result of composing lower alphabet `la`
> with upper alphabet `ua`. First inserts all symbols of `la` then of
> `ua` into this alphabet (insert_symbols). Sets utf8 = la.utf8.
> Builds a lookup table cs: a map from Character to set<Character>.
> Iterate over ua's labels: for each label, let lc = its lower_char;
> if lc == epsilon, insert the label as-is into this alphabet
> (insert(*it)); otherwise add the label's upper_char to the set
> cs[lc]. Then iterate over la's labels: let uc = its upper_char; if
> uc == epsilon, insert the label as-is; otherwise if uc is a key in
> cs, take its set s, let lc = this label's lower_char, and for every
> Character u in s insert a new Label(lc, u) into this alphabet. This
> matches la's upper symbols against ua's lower symbols, producing
> composed lower:upper labels, while epsilon-bearing labels on either
> side pass through unchanged (epsilon labels are still dropped by
> insert()'s is_epsilon guard). Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.compute-score-fn]
> int Alphabet::compute_score( Analysis &ana )

> [spec:hfst:sem:alphabet.sfst.alphabet.compute-score-fn]
> Heuristically scores an Analysis `ana` (German morphology
> heuristic); higher (less negative) is better. score starts at 0.
> Phase 1 (explicit boundaries): for each label i, get its lower
> symbol string via write_char(ana[i].lower_char()); if it equals
> "<X>", decrement score. If after this pass score < 0, return score
> immediately (explicit morpheme boundaries were found).
> Phase 2 (POS/PREF tag counting): otherwise iterate again over each
> label. Get sym = write_char(lower_char). Skip if sym is not a
> multi-character symbol (sym[0] != '<' or sym[1] == 0). If it is a
> POS tag of form "<+...>" (sym[1]=='+'): scan from sym+2 over
> uppercase A-Z; if at least one uppercase letter was consumed and
> the next char is '>', return score immediately. Otherwise test
> whether sym is an all-uppercase tag "<UPPER>": scan from sym+1 over
> A-Z; if no uppercase letters or the terminator after them is not
> '>', skip (continue). For a valid uppercase tag: skip (continue) if
> sym is "<SUFF>", "<OLDORTH>", or "<NEWORTH>". If sym is "<PREF>",
> subtract 2 from score. If sym is "<V>" or "<ADJ>" (is_verb true for
> "<V>"): find the next following label whose lower_char is
> non-epsilon, get its symbol; if that symbol is "<OLDORTH>",
> "<NEWORTH>", or "<SUFF>", skip past it to the next non-epsilon
> symbol and use that instead; then if is_verb and the symbol is
> "<PPres>" or "<PPast>", continue (don't count participles as
> complex); if not verb and the symbol is "<Sup>" or "<Comp>",
> continue. For every remaining counted tag, decrement score by 1.
> Return the final score.

> [spec:hfst:def:alphabet.sfst.alphabet.const-iterator]
> typedef LabelSet::const_iterator const_iterator

> [spec:hfst:def:alphabet.sfst.alphabet.copy-fn]
> void Alphabet::copy( const Alphabet &a )

> [spec:hfst:sem:alphabet.sfst.alphabet.copy-fn]
> Copies the symbols and labels of alphabet `a` into this alphabet
> (additive; does not clear first). Sets utf8 = a.utf8. Reserves
> capacity in sm and cm to a.sm.size() (resize hint). Calls
> insert_symbols(a) to copy all symbol/code mappings (with the
> consistency checks of add_symbol). Then iterates over a's label set
> (a.begin()..a.end()) and inserts each label directly into this->ls
> (ls.insert(*it)) — note this uses raw set insertion, not the
> insert() method, so epsilon labels are not filtered here. Returns
> nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.delete-markers-fn]
> void Alphabet::delete_markers()

> [spec:hfst:sem:alphabet.sfst.alphabet.delete-markers-fn]
> Removes all marker symbols (strings matching ">[0-9]+<", per
> is_marker_symbol) and any labels referencing them, then rebuilds
> the alphabet. Steps: (1) Iterate cm; for each (code c, symbol s)
> where s is NOT a marker symbol, push a duplicated copy of s
> (fst_strdup) into vector `sym` and c into vector `code`. (2)
> Iterate the label set (begin()..end()); keep into vector `label`
> each Label l whose upper_char's symbol and lower_char's symbol
> (via code2symbol) are both non-marker. (3) Call clear() to empty
> the alphabet entirely (frees all owned strings). (4) Re-add the
> kept symbols: for each i call add_symbol(sym[i], code[i]) then
> free(sym[i]) (the temporary duplicate). (5) Re-insert the kept
> labels via insert(label[i]). Returns nothing. Net effect: marker
> codes/symbols and marker-containing labels are gone; surviving
> symbols keep their original codes.

> [spec:hfst:def:alphabet.sfst.alphabet.disambiguate-fn]
> void Alphabet::disambiguate( vector<Analysis> &analyses )

> [spec:hfst:sem:alphabet.sfst.alphabet.disambiguate-fn]
> Filters the vector of analyses in place, keeping only the
> highest-scoring ones. Initialize bestscore = INT_MIN and an empty
> int vector `score`. For each analysis i, compute its score via
> compute_score(analyses[i]), push it into `score`, and update
> bestscore to the maximum seen. Then compact: with write index k=0,
> iterate all i, and whenever score[i] == bestscore copy analyses[i]
> to analyses[k] and increment k. Finally resize analyses to k. After
> this, analyses contains exactly those original analyses whose score
> equals the maximum, in original order. Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.end-fn]
> const_iterator end() const

> [spec:hfst:sem:alphabet.sfst.alphabet.end-fn]
> Const accessor. Returns ls.end(), the past-the-end const_iterator
> of the alphabet's label set ls.

> [spec:hfst:def:alphabet.sfst.alphabet.eqstr]
> struct eqstr

> [spec:hfst:def:alphabet.sfst.alphabet.eqstr.operator-fn]
> bool operator()(const char* s1, const char* s2) const

> [spec:hfst:sem:alphabet.sfst.alphabet.eqstr.operator-fn]
> String-equality functor used as the hash-map key comparator for the
> symbol map. Returns true iff strcmp(s1, s2) == 0, i.e. the two
> C strings are byte-for-byte equal.

> [spec:hfst:def:alphabet.sfst.alphabet.find-fn]
> iterator find( Label l )

> [spec:hfst:sem:alphabet.sfst.alphabet.find-fn]
> Looks up label `l` in the label set ls and returns ls.find(l) — an
> iterator to the matching label, or end() if `l` is not present.
> Matching uses the set's Label::label_cmp ordering.

> [spec:hfst:def:alphabet.sfst.alphabet.get-char-map-fn]
> CharMap get_char_map(void)

> [spec:hfst:sem:alphabet.sfst.alphabet.get-char-map-fn]
> HFST addition. Returns a copy of the code-to-symbol map cm (returned
> by value). The returned CharMap holds the same char* pointers as the
> alphabet's cm (shallow copy of the map entries; the strings are not
> duplicated).

> [spec:hfst:def:alphabet.sfst.alphabet.insert-fn]
> void insert( Label l )

> [spec:hfst:sem:alphabet.sfst.alphabet.insert-fn]
> Inserts label `l` into the label set ls, but only if `l` is not an
> epsilon label (l.is_epsilon() false). If `l` is epsilon (both
> symbols epsilon) it is silently dropped. ls is a set, so duplicates
> are ignored. Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.insert-symbols-fn]
> void Alphabet::insert_symbols( const Alphabet &a )

> [spec:hfst:sem:alphabet.sfst.alphabet.insert-symbols-fn]
> Copies every symbol/code mapping from alphabet `a` into this
> alphabet. Iterates over a.cm and for each entry calls
> add_symbol(it->second /*symbol string*/, it->first /*code*/), so
> the two-argument add_symbol consistency/conflict checks apply (may
> throw on incompatible redefinition). Only symbol mappings are
> copied; the label set is not touched. Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.iterator]
> typedef LabelSet::iterator iterator

> [spec:hfst:def:alphabet.sfst.alphabet.label-set]
> typedef set<Label, Label::label_cmp> LabelSet

> [spec:hfst:def:alphabet.sfst.alphabet.new-marker-fn]
> Character Alphabet::new_marker()

> [spec:hfst:sem:alphabet.sfst.alphabet.new-marker-fn]
> Allocates and registers a fresh marker symbol, returning its code.
> Loops a Character i from 1, incrementing, with loop condition i != 0
> (so it wraps and stops after exhausting all non-zero 16-bit codes).
> For the first i not already present in cm (cm.find(i)==cm.end()):
> formats a unique identifier string ">i<" (sprintf with %ld of i)
> into a 100-byte stack buffer, calls add(symbol, i) to register it,
> and returns i. If no unused code is found (loop wraps to 0), throws
> the C-string "Error: too many symbols in transducer definition".

> [spec:hfst:def:alphabet.sfst.alphabet.next-code-fn]
> int Alphabet::next_code( char* &string, bool extended, bool insert )

> [spec:hfst:sem:alphabet.sfst.alphabet.next-code-fn]
> Scans and returns the code of the next single symbol from the
> C-string pointed to by reference `string`, advancing `string` past
> it. If `*string` is NUL (end), return EOF. Otherwise first try a
> multi-character symbol: c = next_mcsym(string, insert); if c != EOF
> return it (string already advanced). If `extended` is true and the
> current char is a backslash '\\', advance `string` once to strip
> the quotation. Then: if the alphabet is utf8, decode one UTF-8
> codepoint via utf8toint(&string) (which advances string); if it
> returns 0, print "Error in UTF-8 encoding!\n" to stderr and return
> EOF; otherwise convert the codepoint back to a UTF-8 string
> (int2utf8) and return add_symbol(that) (registering it, getting/
> assigning a code). If not utf8, take the single byte *string into a
> 2-char buffer (byte + NUL), advance string by 1, and return
> add_symbol(buffer). Default args: extended=true, insert=true.

> [spec:hfst:def:alphabet.sfst.alphabet.next-label-fn]
> Label Alphabet::next_label( char* &string, bool extended )

> [spec:hfst:sem:alphabet.sfst.alphabet.next-label-fn]
> Scans the next Label from C-string reference `string`, advancing it.
> Read the first code c = next_code(string, extended). If c == EOF,
> return a default Label() (epsilon) signalling end of string. Set
> lc = (Character)c. If not `extended`, or the next char is not ':',
> the label is a single character: if lc == epsilon, recurse
> next_label(string, extended) to skip epsilons; otherwise return
> Label(lc). If extended and next char is ':' (a pair lower:upper):
> advance string past the ':', read the second code c =
> next_code(string, extended); if c == EOF, format an error
> "Error: incomplete symbol in input file: <string>" into a static
> buffer and throw it. Build Label l(lc, (Character)c); if
> l.is_epsilon(), recurse to skip it; otherwise return l. Default
> extended=true. Epsilon labels are thus skipped (consuming further
> input recursively).

> [spec:hfst:def:alphabet.sfst.alphabet.next-mcsym-fn]
> int Alphabet::next_mcsym( char* &string, bool insert )

> [spec:hfst:sem:alphabet.sfst.alphabet.next-mcsym-fn]
> Recognizes a multi-character symbol delimited by angle brackets
> "<...>" at the start of C-string reference `string`. Let start =
> string. Only if *start == '<': scan a pointer end from start+1
> forward; at the first '>' found, treat start..end (inclusive of the
> '>') as a candidate symbol. Pre-increment end to point just past
> '>', temporarily save the char there (lastc) and overwrite it with
> NUL so `start` is a NUL-terminated candidate. If `insert` is true,
> c = add_symbol(start) (registers it, assigns a code); else c =
> symbol2code(start) (lookup only). Restore the saved char (*end =
> lastc). If c != EOF: advance string = end (past the symbol) and
> return (Character)c. If c == EOF (not a known/insertable complex
> symbol), break out of the bracket scan. In all other cases (no
> leading '<', no closing '>', or break), return EOF and leave
> `string` unchanged. Default insert=true.

> [spec:hfst:def:alphabet.sfst.alphabet.operator-fn]
> bool Alphabet::operator==(const Alphabet &alpha) const

> [spec:hfst:sem:alphabet.sfst.alphabet.operator-fn]
> HFST addition: equality test between this alphabet and `alpha`,
> comparing only the symbol maps sm. First loop: for every (symbol,
> code) entry in this->sm, look it up in alpha.sm; if the symbol is
> absent in alpha, return false; if present AND the codes are EQUAL
> (alpha_it->second == it->second), return false. Second loop: the
> symmetric check, iterating alpha.sm against this->sm with the same
> two conditions. If neither loop returned false, return true. NOTE:
> the equal-code branches return false (this looks like an inverted
> condition / bug — it returns false on matching codes rather than
> mismatching ones), but the spec must mirror the C++ exactly: ports
> must replicate this behavior.

> [spec:hfst:def:alphabet.sfst.alphabet.print-analysis-fn]
> char *Alphabet::print_analysis( Analysis &ana, bool both_layers )

> [spec:hfst:sem:alphabet.sfst.alphabet.print-analysis-fn]
> Renders an Analysis `ana` into a printable C string. Build a
> char vector `ch`. For each label l = ana[i]: if `both_layers` is
> true, set s = write_label(l) (the "lower:upper" form, or just one
> char if equal); additionally, if s equals exactly ":" push a
> backslash '\\' into ch first (quoting a bare colon). Else (single
> layer): if l.lower_char() != epsilon, set s = write_char of the
> lower char; otherwise continue (skip epsilon lower symbols). Append
> every character of s to ch. After all labels, push a terminating
> NUL into ch. Then manage a function-static char* `result`: if a
> previous result exists, delete[] it; allocate result = new
> char[ch.size()] and copy ch into it. Return result. Side effect:
> the returned pointer is owned by the static and is invalidated by
> the next call (not thread-safe).

> [spec:hfst:def:alphabet.sfst.alphabet.print-fn]
> void Alphabet::print(void)

> [spec:hfst:sem:alphabet.sfst.alphabet.print-fn]
> HFST addition / debug print. Iterates over the code map cm and
> writes one line per entry to stderr in the format "%i\t%s\n", i.e.
> the numeric code (it->first) then a tab then the symbol string
> (it->second). Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.read-fn]
> void Alphabet::read( FILE *file )

> [spec:hfst:sem:alphabet.sfst.alphabet.read-fn]
> Reads an alphabet from binary `file` (inverse of store). Read one
> byte; set utf8 = (byte != 0). Read a Character count n (read_num).
> Loop n times to read the symbol mapping: each iteration uses a
> stack buffer of BUFFER_SIZE (100000), reads a Character code c
> (read_num), then reads a NUL-terminated symbol string into buffer
> (read_string); if read_string fails or feof/ferror, throw "Error1
> occurred while reading alphabet!\n". HFST addition: if the read
> string is empty (""), throw "Empty string cannot be a symbol in
> HFST!\n". Otherwise call add_symbol(buffer, c). After the symbol
> table, read another Character count n; if ferror, throw "Error2
> occurred while reading alphabet!\n". Loop n times reading two
> Characters lc, uc (read_num each) and insert(Label(lc, uc)) into
> the label set. Finally if ferror, throw "Error3 occurred while
> reading alphabet!\n". Returns nothing. Mutates utf8, sm, cm, ls.

> [spec:hfst:def:alphabet.sfst.alphabet.size-fn]
> size_t size() const

> [spec:hfst:sem:alphabet.sfst.alphabet.size-fn]
> Const accessor. Returns ls.size(), the number of labels (character
> pairs) currently in the alphabet's label set.

> [spec:hfst:def:alphabet.sfst.alphabet.store-fn]
> void Alphabet::store( FILE *file ) const

> [spec:hfst:sem:alphabet.sfst.alphabet.store-fn]
> Writes the alphabet to `file` in binary form (inverse of read).
> First write one byte: 1 if utf8 else 0 (fputc). Then write the
> symbol mapping: write a Character n = cm.size() (fwrite), then for
> each (code c, symbol s) in cm write the Character c followed by the
> symbol string including its terminating NUL (strlen(s)+1 bytes).
> Then write the character pairs: write a Character n = size() (number
> of labels in ls), and for each label in ls write its lower_char
> then its upper_char (each a Character). Finally if ferror(file),
> throw "Error encountered while writing alphabet to file\n". Returns
> nothing. Note ordering follows cm and ls iteration order
> respectively.

> [spec:hfst:def:alphabet.sfst.alphabet.string2labelseq-fn]
> void Alphabet::string2labelseq( char *s, vector<Label> &labels )

> [spec:hfst:sem:alphabet.sfst.alphabet.string2labelseq-fn]
> Converts C-string `s` into a sequence of Labels appended to
> `labels`. Repeatedly calls next_label(s) (default extended=true,
> which advances s and may insert symbols/throw on incomplete pairs)
> and, while the returned label is not Label::epsilon (the end-of-
> string sentinel returned by next_label), pushes it onto `labels`.
> Stops when next_label returns epsilon (end reached). Returns
> nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.string2symseq-fn]
> void Alphabet::string2symseq( char *s, vector<Character> &ch )

> [spec:hfst:sem:alphabet.sfst.alphabet.string2symseq-fn]
> Converts C-string `s` into a sequence of Character codes appended to
> `ch`. Repeatedly calls next_code(s, false, false) — i.e. with
> extended=false (no backslash quoting, no ':' pairs) and insert=false
> (lookup only, do not register new symbols) — which advances `s` past
> each scanned symbol. While the returned code c is not EOF, push
> (Character)c onto `ch`. Stops when next_code returns EOF (end of
> string). Returns nothing.

> [spec:hfst:def:alphabet.sfst.alphabet.symbol-map]
> typedef hash_map<const char*, Character, hash<const char*>,eqstr> SymbolMap

> [spec:hfst:def:alphabet.sfst.alphabet.symbol2code-fn]
> int symbol2code( const char *s ) const

> [spec:hfst:sem:alphabet.sfst.alphabet.symbol2code-fn]
> Const lookup. Searches the symbol map sm for the C-string key `s`
> (compared by byte-equality via the eqstr functor). If found, returns
> the associated Character code (p->second) as an int. If the symbol is
> not present, returns EOF.

> [spec:hfst:def:alphabet.sfst.alphabet.write-char-fn]
> void Alphabet::write_char( Character c, char *buffer, int *pos,

> [spec:hfst:sem:alphabet.sfst.alphabet.write-char-fn]
> Appends a printable rendering of Character code `c` into `buffer` at
> offset `*pos`, advancing `*pos`, and NUL-terminates. Look up s =
> code2symbol(c). If s is non-NULL (known symbol): set i=0 and
> l=strlen(s)-1; if !with_brackets and s starts with '<' and ends with
> '>', strip the brackets by incrementing i and decrementing l; then
> copy s[i..=l] byte-by-byte into buffer at the advancing *pos. If s is
> NULL (unknown code): let uc = (unsigned)c; if 32 <= uc < 256 write the
> single byte (char)c into buffer; otherwise sprintf "\<uc>" (a
> backslash followed by the decimal code) into buffer at *pos and
> advance *pos by the length written. Finally always set buffer[*pos] =
> '\0' (without advancing *pos past it). Returns nothing; mutates
> buffer and *pos. (The two-arg overload write_char(c, with_brackets)
> renders into a static 1000-byte buffer starting at n=0 and returns it.)

> [spec:hfst:def:alphabet.sfst.alphabet.write-label-fn]
> void Alphabet::write_label( Label l, char *buffer, int *pos,

> [spec:hfst:sem:alphabet.sfst.alphabet.write-label-fn]
> Appends a printable rendering of Label `l` into `buffer` at offset
> `*pos`, advancing `*pos`. Let lc = l.lower_char() and uc =
> l.upper_char(). First write_char(lc, buffer, pos, with_brackets) to
> emit the lower symbol. If lc != uc, write a ':' separator into buffer
> at the advancing *pos, then write_char(uc, buffer, pos,
> with_brackets) to emit the upper symbol. If lc == uc, only the single
> symbol is written (no colon). Returns nothing; mutates buffer and
> *pos. (The two-arg overload write_label(l, with_brackets) renders into
> a static 1000-byte buffer starting at n=0 and returns it.)

> [spec:hfst:def:alphabet.sfst.analysis]
> typedef vector<Label> Analysis

> [spec:hfst:def:alphabet.sfst.character]
> typedef unsigned short Character

> [spec:hfst:def:alphabet.sfst.is-marker-symbol-fn]
> static bool is_marker_symbol( const char *s )

> [spec:hfst:sem:alphabet.sfst.is-marker-symbol-fn]
> Static predicate. Returns true iff C-string `s` matches the pattern
> ">[0-9]+<" (a '>' then one or more decimal digits then a '<' at the
> very end). Steps: if s is non-NULL and *s == '>', advance s once and
> then keep advancing while the current char is a digit '0'..'9' (a
> do/while, so it always advances at least once past the '>'). After
> the digit run, return true iff the current char is '<', the char
> after it is NUL (end of string), AND the char before it (*(s-1)) is
> not '>' (i.e. at least one digit was consumed). Otherwise return
> false (including when s is NULL or does not start with '>').

> [spec:hfst:def:alphabet.sfst.label]
> class Label {
>   struct { Character lower; Character upper; } label;
>   static const Character epsilon=0;
>   struct label_hash { // [spec:hfst:def:alphabet.sfst.label.label-hash.operator-fn] // [spec:hfst:sem:alphabet.sfst.label.label-hash.operator-fn] size_t operat...;
>   struct label_cmp { // [spec:hfst:def:alphabet.sfst.label.label-cmp.operator-fn] // [spec:hfst:sem:alphabet.sfst.label.label-cmp.operator-fn] bool operator() ...;
>   struct label_eq { // [spec:hfst:def:alphabet.sfst.label.label-eq.operator-fn] // [spec:hfst:sem:alphabet.sfst.label.label-eq.operator-fn] bool operator() ( c...;
> }

> [spec:hfst:def:alphabet.sfst.label.get-char-fn]
> Character get_char( Level l ) const

> [spec:hfst:sem:alphabet.sfst.label.get-char-fn]
> Const accessor selecting one symbol of the label by Level `l`. The
> Level enum is {upper=0, lower=1}. Returns label.upper if l == upper,
> otherwise (l == lower) returns label.lower.

> [spec:hfst:def:alphabet.sfst.label.is-epsilon-fn]
> int is_epsilon() const

> [spec:hfst:sem:alphabet.sfst.label.is-epsilon-fn]
> Const predicate. Returns true (nonzero) iff BOTH the label's upper
> and lower symbols equal epsilon (Label::epsilon == 0): label.upper ==
> epsilon && label.lower == epsilon.

> [spec:hfst:def:alphabet.sfst.label.label-cmp]
> struct label_cmp

> [spec:hfst:def:alphabet.sfst.label.label-cmp.operator-fn]
> bool operator() ( const Label l1, const Label l2 ) const

> [spec:hfst:sem:alphabet.sfst.label.label-cmp.operator-fn]
> Strict-weak-ordering comparator functor for labels (used to order the
> LabelSet `set<Label, label_cmp>`). Returns true iff l1 sorts before
> l2 by (lower_char, upper_char) lexicographic order: true if
> l1.lower_char() < l2.lower_char(), OR if the lower chars are equal and
> l1.upper_char() < l2.upper_char(); false otherwise. Note this orders
> by lower symbol FIRST then upper (unlike Label::operator< which orders
> by upper first).

> [spec:hfst:def:alphabet.sfst.label.label-eq]
> struct label_eq

> [spec:hfst:def:alphabet.sfst.label.label-eq.operator-fn]
> bool operator() ( const Label l1, const Label l2 ) const

> [spec:hfst:sem:alphabet.sfst.label.label-eq.operator-fn]
> Equality comparator functor for labels (used as the hash-table key
> equality predicate). Returns true iff both symbols match: l1 and l2
> have equal lower_char() AND equal upper_char().

> [spec:hfst:def:alphabet.sfst.label.label-fn]
> Label( Character c1, Character c2 )

> [spec:hfst:sem:alphabet.sfst.label.label-fn]
> Two-argument Label constructor. Stores c1 as the lower symbol
> (label.lower = c1) and c2 as the upper symbol (label.upper = c2). No
> validation or normalization. (The single-arg constructor Label(c=0)
> instead sets both lower and upper to the same c, defaulting to
> epsilon.)

> [spec:hfst:def:alphabet.sfst.label.label-hash]
> struct label_hash

> [spec:hfst:def:alphabet.sfst.label.label-hash.operator-fn]
> size_t operator() ( const Label l ) const

> [spec:hfst:sem:alphabet.sfst.label.label-hash.operator-fn]
> Hash functor for labels. Computes a size_t hash by XORing three
> terms: (size_t)lower_char(), (size_t)upper_char() shifted left by 16
> bits, and (size_t)upper_char() shifted right by 16 bits. I.e.
> lower ^ (upper << 16) ^ (upper >> 16). Returns that value.

> [spec:hfst:def:alphabet.sfst.label.lower-char-fn]
> Character lower_char() const

> [spec:hfst:sem:alphabet.sfst.label.lower-char-fn]
> Const accessor. Returns the label's lower (analysis-level) symbol,
> label.lower.

> [spec:hfst:def:alphabet.sfst.label.lower-is-epsilon-fn]
> int lower_is_epsilon() const

> [spec:hfst:sem:alphabet.sfst.label.lower-is-epsilon-fn]
> Const predicate. Returns true (nonzero) iff the label's lower symbol
> equals epsilon (0): label.lower == epsilon.

> [spec:hfst:def:alphabet.sfst.label.operator-fn]
> int operator<( Label l ) const

> [spec:hfst:sem:alphabet.sfst.label.operator-fn]
> Less-than operator on labels, ordering by upper symbol first then
> lower (used for sorting in compact.C). Returns true if
> upper_char() < l.upper_char(); returns false if upper_char() >
> l.upper_char(); otherwise (upper chars equal) returns true iff
> lower_char() < l.lower_char(), else false. Note the matching
> operator> mirrors this with > comparisons.

> [spec:hfst:def:alphabet.sfst.label.replace-char-fn]
> Label replace_char( Character c, Character nc ) const

> [spec:hfst:sem:alphabet.sfst.label.replace-char-fn]
> Const; returns a NEW Label that is a copy of this one with every
> occurrence of character `c` replaced by `nc`. Copies *this into a
> local l; if l.label.lower == c set l.label.lower = nc; independently
> if l.label.upper == c set l.label.upper = nc; returns l. Does not
> mutate the receiver. Both symbols are checked independently, so a
> label with c on both sides has both replaced.

> [spec:hfst:def:alphabet.sfst.label.upper-char-fn]
> Character upper_char() const

> [spec:hfst:sem:alphabet.sfst.label.upper-char-fn]
> Const accessor. Returns the label's upper (surface-level) symbol,
> label.upper.

> [spec:hfst:def:alphabet.sfst.label.upper-is-epsilon-fn]
> int upper_is_epsilon() const

> [spec:hfst:sem:alphabet.sfst.label.upper-is-epsilon-fn]
> Const predicate. Returns true (nonzero) iff the label's upper symbol
> equals epsilon (0): label.upper == epsilon.

> [spec:hfst:def:alphabet.sfst.level]
> typedef enum

