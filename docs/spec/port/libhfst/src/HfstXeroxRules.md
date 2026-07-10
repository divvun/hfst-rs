# libhfst/src/HfstXeroxRules.cc, libhfst/src/HfstXeroxRules.h

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.after-fn]
> HfstTransducer after( const HfstTransducer &left, const HfstTransducer &right)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.after-fn]
> Implements the "a > b" (a occurs after b... actually a before b in surface order) restriction over automata. First it checks that both `left` and `right` are automata (acceptors): for each, build an input-projection copy and an output-projection copy and compare each to the original; if any of the four comparisons fails (i.e. either operand is not identical to both its projections), throw TransducersAreNotAutomataException with message "HfstXeroxRules::restriction".
> Let `type = left.get_type()`. Build `identity` = the identity-pair transducer repeated star (`?*`), optimized.
> Build `tmp` = identity copy concatenated with `left`, then `identity`, then `right`, then `identity`, optimized (i.e. `?* left ?* right ?*`).
> Build `retval` = a fresh `identity` copy, subtract `tmp` from it, optimize, and return it (i.e. `?* - [?* left ?* right ?*]`).

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.apply-boundary-mark-fn]
> HfstTransducer applyBoundaryMark( const HfstTransducer &t )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.apply-boundary-mark-fn]
> Applies the start/end boundary marker `.#.` to transducer `t` and then strips it. Set up a tokenizer with multichar symbols `@_EPSILON_SYMBOL_@`, `@_UNKNOWN_SYMBOL_@`, `@TMP_UNKNOWN@`, and `.#.`. Let `type = t.get_type()`. Build `boundary` = the `.#.` transducer.
> Build `identityPair` = the identity-pair transducer with `.#.` inserted into its alphabet. Build `identityMinusBoundary` = identityPair minus `boundary` (i.e. `? - .#.`), and `identityMinusBoundaryStar` = that repeated star.
> Build `boundaryAnythingBoundary` = `boundary . identityMinusBoundaryStar . boundary` (i.e. `.#. (?-.#.)* .#.`), optimized.
> Build `retval` = `[0:.#. | ?-.#.]*`: disjunct `zeroToBoundary` (epsilon-to-`.#.`) with `identityMinusBoundary`, optimize, repeat star, optimize.
> Build `removeBoundary` = `[.#.:0 | ?-.#.]*`: disjunct `boundaryToZero` (`.#.`-to-epsilon) with `identityMinusBoundary`, optimize, repeat star, optimize.
> Copy `t` into `tr` and substitute every `@_UNKNOWN_SYMBOL_@` with `@TMP_UNKNOWN@` (this protects unknowns through the first composition). Compose `retval` with `tr` and optimize (prepends optional boundary insertion). Then compose `retval` with `boundaryAnythingBoundary` and optimize (requires exactly one leading and one trailing boundary). Then compose with `removeBoundary` and optimize (removes the boundaries again).
> Finally substitute `@TMP_UNKNOWN@` back to `@_UNKNOWN_SYMBOL_@`, remove `@TMP_UNKNOWN@` from the alphabet, remove `.#.` from the alphabet, and return `retval`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.before-fn]
> HfstTransducer before( const HfstTransducer &left, const HfstTransducer &right)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.before-fn]
> Implements the "a < b" before restriction over automata. First verify both `left` and `right` are automata: for each operand build an input-projection copy and an output-projection copy and compare each to the original; if any of the four comparisons fails, throw TransducersAreNotAutomataException with message "HfstXeroxRules::restriction".
> Let `type = left.get_type()`. Build `identity` = identity-pair repeated star (`?*`), optimized.
> Build `tmp` = identity copy concatenated with `right`, then `identity`, then `left`, then `identity`, optimized (i.e. `?* right ?* left ?*` — note `right` comes first here, the opposite ordering of `after`).
> Build `retval` = a fresh `identity` copy, subtract `tmp`, optimize, and return.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.bracketed-replace-fn]
> HfstTransducer bracketedReplace(const Rule &rule, bool optional)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.bracketed-replace-fn]
> Core unconditional/conditional bracketed replace for a single `Rule`. Set up a tokenizer with multichar symbols `@_EPSILON_SYMBOL_@`, `@_UNKNOWN_SYMBOL_@`, and the markers `@LM@`, `@RM@`, `@TMPM@`, `@LM2@`, `@RM2@`, `$Epsilon$`, `.#.`.
> Copy `rule` into `ruletmp` and call `ruletmp.encodeFlags()` (encodes flag diacritics in mappings and contexts). Extract `mappingPairVector`, `ContextVector`, `replType` from `ruletmp`. Let `type` = type of the first mapping's first transducer. Build `identity` = identity-pair repeated star, optimized.
> Build the combined `mapping` transducer: for each mapping pair i, copy its `.first` into `oneMappingPair`; unless its property "isMarkup" == "yes", cross-product with the pair's `.second` (markup pairs are already a cross product). Build `removeHash` = `identityWithoutBoundary . .#. . identityWithoutBoundary` (identity star with `.#.` in its alphabet), used to remove any path containing `.#.` in the center: subtract `removeHash` (non-harmonizing) from `oneMappingPair`, remove `.#.` from its alphabet. For i==0 assign to `mapping`; otherwise disjunct into `mapping` and optimize.
> Edge case ? -> x: if `mapping` equals the empty transducer, set `mapping = identity`; and if the first mapping pair's `.second` is also empty, insert every symbol from the first pair's `.first` alphabet into `mapping`'s alphabet.
> Insert `@LM@`, `@RM@`, `@TMPM@` into `mapping`'s alphabet. Build `mappingWithBrackets` = `@LM@ . mapping . @RM@`.
> If not optional: build `leftMappingUnion` = union of every mapping pair's `.first`, insert the marker symbols into its alphabet, build `mappingWithBrackets2` = `@LM2@ . leftMappingUnion . @RM2@`, insert `@LM2@`/`@RM2@` into `mappingWithBrackets`'s alphabet, and disjunct `mappingWithBrackets2` into `mappingWithBrackets`.
> Build `identityExpanded` = identity-pair with `@LM@`,`@RM@`,`@TMPM@` (and, if non-optional, `@LM2@`,`@RM2@`) in alphabet, disjunct `mappingWithBrackets`, optimize, repeat star, optimize — i.e. `[I:I | <a:b>]*`.
> If there is exactly one context and it is the epsilon/epsilon context (`ContextVector[0].first` and `.second` both equal epsilon), then remove `@TMPM@` from `identityExpanded`'s alphabet and return `identityExpanded` directly (no contexts).
> Otherwise build `mappingWithBracketsAndTmpBoundary` = `@TMPM@ . mappingWithBrackets . @TMPM@`. Build `bracketedReplace` = `identityExpanded . mappingWithBracketsAndTmpBoundary . identityExpanded` (`.* |<a:b>| .*`).
> Call `expandContextsWithMapping(ContextVector, mappingWithBracketsAndTmpBoundary, identityExpanded, replType, optional)` to get `unionContextReplace`. Compute `replaceWithoutContexts` = `bracketedReplace` minus `unionContextReplace`, optimized. Substitute `@TMPM@` with epsilon, remove `@TMPM@` from its alphabet, optimize. Remove `@TMPM@` from `identityExpanded`'s alphabet.
> Final negation: `uncondidtionalTr` = `identityExpanded` minus `replaceWithoutContexts`, optimized; return it.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.constraint-composition-fn]
> HfstTransducer constraintComposition( const HfstTransducer &t, const HfstTransducer &Constraint )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.constraint-composition-fn]
> Implements generalized lenient composition of `t` with `Constraint`. Copy `t` into `retval`, transform all its weights to zero (via `zero_weight`), input-project it and optimize (so `retval` becomes the input language of `t` with zero weights).
> Build `tmp` = copy of `retval`; compose with `Constraint` and optimize; compose with `retval` again and optimize; then output-project and optimize (`tmp = (t.1 .o. Constraint .o. t.1).2`). Subtract `tmp` from `retval` and optimize (`retval = t.1 - tmp.2`).
> Transform `retval`'s weights to zero again, compose with the original `t` and optimize. Return `retval` (`(t.1 - tmp.2) .o. t`).

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.constraints-right-part-fn]
> HfstTransducer constraintsRightPart( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.constraints-right-part-fn]
> Helper that builds and returns `[ B:0 | 0:B | ?-B ]*` for the given `type`, where `B = @LM@ | @RM@`. Set up a tokenizer with multichar symbols `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`. Build `identityPair` = identity-pair. Build `leftBracket`/`rightBracket` from `@LM@`/`@RM@`, and `B` = their disjunction, optimized.
> Build `epsilonToBrackets` = epsilon cross-product `B` (i.e. `0:B`), and `bracketsToEpsilon` = `B` cross-product epsilon (i.e. `B:0`). Build `identityPairMinusBrackets` = `identityPair - B`, optimized (`?-B`).
> Build `rightPart` = `epsilonToBrackets` disjunct `bracketsToEpsilon` disjunct `identityPairMinusBrackets`, optimize, repeat star, optimize. Return `rightPart`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.create-mapping-for-mark-up-replace-fn]
> HfstTransducerPair create_mapping_for_mark_up_replace( const HfstTransducerPair &mappingPair,

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.create-mapping-for-mark-up-replace-fn]
> Builds a markup-replace mapping pair from a `mappingPair` and a `marks` pair (left/right mark transducers). Set up a tokenizer with `@_EPSILON_SYMBOL_@` as a multichar symbol. Let `type` = type of `mappingPair.first`. Take `leftMark = marks.first`, `rightMark = marks.second`.
> Build `epsilonToLeftMark` = epsilon transducer cross-product `leftMark`, optimized (`0:leftMark`); build `epsilonToRightMark` = epsilon transducer cross-product `rightMark`, optimized (`0:rightMark`).
> Build `mappingCrossProduct` = `epsilonToLeftMark . mappingPair.first . epsilonToRightMark`, optimized, and set its property "isMarkup" = "yes".
> Construct and return an HfstTransducerPair whose `.first` is `mappingCrossProduct` and whose `.second` is a plain epsilon transducer.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.decode-flag-diacritics-fn]
> HfstTransducer decodeFlagDiacritics( const HfstTransducer &tr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.decode-flag-diacritics-fn]
> Inverse of encodeFlagDiacritics: turns "fake" flag symbols back into real flag diacritics. Iterate over `tr.get_alphabet()`. For each symbol, examine its first 3 characters; if they match one of `$P.`, `$R.`, `$U.`, `$D.`, `$C.`, `$N.` or their lowercase variants `$p.`,`$r.`,`$u.`,`$d.`,`$c.`,`$n.`, copy the symbol and replace every `$` with `@`, record a substitution from the original symbol to the new symbol in `fakeFlagsToRealFlags`, and add the original to `removeFromAlphabet`.
> Copy `tr` into `retval`, apply the substitutions, remove the `removeFromAlphabet` symbols from `retval`'s alphabet, and return `retval`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.disjunct-vector-members-fn]
> HfstTransducer disjunctVectorMembers( const HfstTransducerVector &trVector )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.disjunct-vector-members-fn]
> Disjuncts (unions) all transducers in `trVector` into one. Initialize `retval` = copy of `trVector[0]`. For each i from 1 to size-1, disjunct `trVector[i]` into `retval` and optimize. Return `retval`. (Assumes the vector is non-empty.)

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.encode-flag-diacritics-fn]
> HfstTransducer encodeFlagDiacritics( const HfstTransducer &tr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.encode-flag-diacritics-fn]
> Turns real flag diacritics into "non-special" multichar symbols by replacing the leading/trailing `@` with `$` (so `@P.FOO.BAR@` becomes `$P.FOO.BAR$`). Iterate over `tr.get_alphabet()`. For each symbol, examine its first 3 characters; if they match one of `@P.`, `@R.`, `@U.`, `@D.`, `@C.`, `@N.` or their lowercase variants `@p.`,`@r.`,`@u.`,`@d.`,`@c.`,`@n.`, copy the symbol and replace every `@` with `$`, record a substitution from the original symbol to the new symbol in `realFlagstoFakeFlags`, and add the original to `removeFromAlphabet`.
> Copy `tr` into `retval`, apply the substitutions, remove the `removeFromAlphabet` symbols from `retval`'s alphabet, and return `retval`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.expand-contexts-with-mapping-fn]
> HfstTransducer expandContextsWithMapping (const HfstTransducerPairVector &ContextVector,

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.expand-contexts-with-mapping-fn]
> For each context pair in `ContextVector`, builds the bracketed-and-expanded `Lc mapping Rc` and disjuncts them all into `unionContextReplace` (which it returns). Let `type = identityExpanded.get_type()`. Initialize `unionContextReplace` empty of that type.
> For each context i: build `identityStar` = identity-pair repeated star. Build `firstContext` (the left context Lc) = `identityStar . ContextVector[i].first`, zero its weights, optimize, then call `insertFreelyAllTheBrackets(firstContext, optional)`. Build `secondContext` (right context Rc) = `ContextVector[i].second . identityStar`, zero weights, optimize, then `insertFreelyAllTheBrackets(secondContext, optional)`.
> Build `leftContextExpanded`/`rightContextExpanded` depending on `replType`, composing the contexts with `identityExpanded` ([I:I | <a:b>]*) on the appropriate side: REPL_UP -> left=firstContext compose identityExpanded, right=secondContext compose identityExpanded; REPL_RIGHT -> left=identityExpanded compose firstContext, right=secondContext compose identityExpanded; REPL_LEFT -> left=firstContext compose identityExpanded, right=identityExpanded compose secondContext; REPL_DOWN -> left=identityExpanded compose firstContext, right=identityExpanded compose secondContext. Zero weights of both expanded contexts and optimize.
> Disjunct `leftContextExpanded` into `firstContext`, optimize; disjunct `rightContextExpanded` into `secondContext`, optimize (so `Cl = Cl | Cl'` and `Cr = Cr | Cr'`).
> Add boundary symbol `.#.`: insert `.#.` into `identityStar`'s alphabet. If `firstContext`'s alphabet has no `.#.`, insert `.#.` into its alphabet and prepend `.#. . identityStar` (i.e. set `firstContext = boundary . identityStar . firstContext`). If `secondContext`'s alphabet has no `.#.`, insert `.#.` into its alphabet and append `. identityStar . boundary`.
> Build `oneContextReplace` = `firstContext . mappingWithBracketsAndTmpBoundary . secondContext`, zero its weights, disjunct into `unionContextReplace`, optimize. After the loop return `unionContextReplace`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.get-marker-number-fn]
> static unsigned int getMarkerNumber(const std::string & str)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.get-marker-number-fn]
> Nominally meant to parse the number N out of a string of form "@N@". It extracts the substring from index 1 with length `str.size()-2` (stripping the leading and trailing `@`). However, due to a known bug (the istringstream is default-constructed and never fed the substring, and a comment notes it cannot be fixed without breaking existing HfstXeroxRules tests), the function ignores the parsed value entirely and unconditionally returns the constant `100000`. A faithful port must reproduce this: always return 100000.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.get-marker-string-fn]
> static std::string getMarkerString(unsigned int i)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.get-marker-string-fn]
> Returns the string `"@" + decimal-string-of-i + "@"`, i.e. formats the unsigned integer `i` into its base-10 textual representation and wraps it in `@...@`. For example i=3 yields "@3@".

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.insert-freely-all-the-brackets-fn]
> void insertFreelyAllTheBrackets( HfstTransducer &t, bool optional )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.insert-freely-all-the-brackets-fn]
> Mutates `t` in place by freely inserting the bracket markers. Set up a tokenizer with multichar symbols `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`, `@LM2@`, `@RM2@`. Let `type = t.get_type()`.
> Build single-symbol transducers `leftBracket`=`@LM@` and `rightBracket`=`@RM@`. Call `t.insert_freely(leftBracket, false)` and optimize, then `t.insert_freely(rightBracket, false)` and optimize (the `false` disables harmonization).
> If `optional` is false, additionally build `@LM2@` and `@RM2@` bracket transducers and freely insert each into `t` (optimizing after each).

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.left-most-constraint-fn]
> HfstTransducer leftMostConstraint( const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.left-most-constraint-fn]
> Builds the left-most constraint `.#. ?* <:0 [B:0]* [I-B] [ B:0 | 0:B | ?-B ]* .#.` and applies it. Set up a tokenizer with `@_EPSILON_SYMBOL_@`, `@_UNKNOWN_SYMBOL_@`, `.#.`, `@LM@`, `@RM@`. Let `type = uncondidtionalTr.get_type()`.
> Build `identity` = identity-pair star. Build `rightPart` via `constraintsRightPart(type)` (`[B:0|0:B|?-B]*`). Build `B = @LM@ | @RM@`. Build `bracketsToEpsilonStar` = `B` cross-product epsilon, optimized, repeat star (`[B:0]*`). Build `identityPairMinusBrackets` = `identityPair - B` (`I-B`). Build `LeftBracketToEpsilon` = `@LM@:0`. Build `boundary` = `.#.`.
> Assemble `Constraint` = `boundary . identity . LeftBracketToEpsilon . bracketsToEpsilonStar . identityPairMinusBrackets . rightPart`, optimized, then concatenate `boundary`, optimized (giving `.#. ?* <:0 [B:0]* [I-B] [B:0|0:B|?-B]* .#.`).
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.longest-match-left-most-constraint-fn]
> HfstTransducer longestMatchLeftMostConstraint( const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.longest-match-left-most-constraint-fn]
> Builds the longest-match left-most constraint and applies it (intended to be composed onto a left-most transducer). Set up a tokenizer with `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`. Let `type = uncondidtionalTr.get_type()`.
> Build `identity` = identity-pair star, `B = @LM@ | @RM@`, `identityPairMinusBrackets = identityPair - B` and its plus-repetition `identityPairMinusBracketsPlus` (`[?-B]+`), and `rightPart` via `constraintsRightPart(type)`. Build the bracket-edit transducers `RightBracketToEpsilon` (`>:0`), `epsilonToRightBracket` (`0:>`), `LeftBracketToEpsilon` (`<:0`), `epsilonToLeftBracket` (`0:<`).
> Build `nonClosingBracketInsertion` = `epsilonToLeftBracket | LeftBracketToEpsilon | epsilonToRightBracket | B`, optimized, then concatenate `identityPairMinusBracketsPlus` (i.e. `[ 0:< | <:0 | 0:> | B ] [?-B]+`). Build `middlePart` = `identityPairMinusBrackets | nonClosingBracketInsertion`, optimized.
> Assemble `Constraint` = `identity . leftBracket . identityPairMinusBracketsPlus . epsilonToRightBracket . middlePart . rightPart`, optimized (i.e. `?* < [?-B]+ 0:> [middle] [B:0|0:B|?-B]*`).
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.longest-match-right-most-constraint-fn]
> HfstTransducer longestMatchRightMostConstraint(const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.longest-match-right-most-constraint-fn]
> Builds the longest-match right-most constraint and applies it. Same tokenizer (`@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`) and type as the left-most variant. Build `identity`, `B = @LM@ | @RM@`, `identityPairMinusBrackets = identityPair - B`, `identityPairMinusBracketsPlus` (`[?-B]+`), `rightPart` via `constraintsRightPart(type)`, and the bracket-edit transducers `RightBracketToEpsilon` (`>:0`), `epsilonToRightBracket` (`0:>`), `LeftBracketToEpsilon` (`<:0`), `epsilonToLeftBracket` (`0:<`).
> Build `nonClosingBracketInsertion` = `identityPair | epsilonToLeftBracket | RightBracketToEpsilon | epsilonToRightBracket | B`, optimized.
> Assemble `Constraint` = `rightPart . identityPairMinusBracketsPlus . nonClosingBracketInsertion`, optimized, then `. epsilonToLeftBracket . identityPairMinusBracketsPlus . rightBracket . identity`, optimized (i.e. `[B:0|0:B|?-B]* [?-B]+ [middle] 0:< [?-B]+ > ?*`).
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.most-brackets-plus-constraint-fn]
> HfstTransducer mostBracketsPlusConstraint( const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.most-brackets-plus-constraint-fn]
> Builds the constraint `?* [ BL:0 (?-B)+ BR:0 ?* ]+` and applies it. Set up a tokenizer with `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`, `@LM2@`, `@RM2@`. Let `type = uncondidtionalTr.get_type()`. Build `identityStar` = identity-pair star.
> Build `allLeftBracketsToEpsilon` = `@LM@:0 | @LM2@:0` (BL:0) and `allRightBracketsToEpsilon` = `@RM@:0 | @RM2@:0` (BR:0). Build `B = @LM@ | @RM@ | @LM2@ | @RM2@`. Build `identityPairMinusBracketsPlus` = `(identityPair - B)` repeated plus (`(?-B)+`).
> Build `repeatingPart` = `allLeftBracketsToEpsilon . identityPairMinusBracketsPlus . allRightBracketsToEpsilon . identityStar`, optimized, then repeat plus (`[ BL:0 (?-B)+ BR:0 ?* ]+`). Assemble `Constraint` = `identityStar . repeatingPart`, optimized.
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.most-brackets-star-constraint-fn]
> HfstTransducer mostBracketsStarConstraint( const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.most-brackets-star-constraint-fn]
> Identical to mostBracketsPlusConstraint except the inner `(?-B)` repetition is star instead of plus. Builds `?* [ BL:0 (?-B)* BR:0 ?* ]+` and applies it. Same setup: tokenizer with `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`, `@LM2@`, `@RM2@`; `identityStar` = identity-pair star; `allLeftBracketsToEpsilon` = `@LM@:0 | @LM2@:0`; `allRightBracketsToEpsilon` = `@RM@:0 | @RM2@:0`; `B = @LM@ | @RM@ | @LM2@ | @RM2@`.
> Build `identityPairMinusBracketsStar` = `(identityPair - B)` repeated star (`(?-B)*`). Build `repeatingPart` = `allLeftBracketsToEpsilon . identityPairMinusBracketsStar . allRightBracketsToEpsilon . identityStar`, optimized, repeat plus. Assemble `Constraint` = `identityStar . repeatingPart`, optimized.
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.no-repetition-constraint-fn]
> HfstTransducer noRepetitionConstraint( const HfstTransducer &t )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.no-repetition-constraint-fn]
> Prevents repeated adjacent empty brackets (used for empty/epenthesis replace rules). Set up a tokenizer with `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`. Determine `optional`: scan `t.get_alphabet()`; if it contains `@LM2@`, set `optional = false` (the non-optional bracket markers are present), else `optional` stays true. Add `@LM2@`, `@RM2@` to the tokenizer. Let `type = t.get_type()`.
> Build `leftBrackets` = `@LM@` (disjunct `@LM2@` if not optional) and `rightBrackets` = `@RM@` (disjunct `@RM2@` if not optional). Build `identityStar` = identity-pair star.
> Assemble `Constraint` = `identityStar . leftBrackets . rightBrackets . leftBrackets . rightBrackets . identityStar`, optimized (matches `?* B< B> B< B> ?*`, i.e. two adjacent empty bracket pairs).
> Return `constraintComposition(t, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.one-betterthan-none-constraint-fn]
> HfstTransducer oneBetterthanNoneConstraint( const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.one-betterthan-none-constraint-fn]
> Builds the constraint `.#. ?* <:0 >:0 ?* .#.` (filters out the empty replacement so a non-empty match is preferred) and applies it. Set up a tokenizer with `@_EPSILON_SYMBOL_@`, `.#.`, `@LM@`, `@RM@`. Let `type = uncondidtionalTr.get_type()`. Build `identity` = identity-pair star, `leftBracketToZero` = `@LM@:0`, `rightBracketToZero` = `@RM@:0`, `boundary` = `.#.`.
> Assemble `Constraint` = `boundary . identity . leftBracketToZero . rightBracketToZero . boundary . identity`, optimized.
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.parallel-bracketed-replace-fn]
> HfstTransducer parallelBracketedReplace

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.parallel-bracketed-replace-fn]
> Bracketed replace for a vector of parallel rules. To keep overlapping mappings with different weights/contexts separate, a per-rule marker symbol is concatenated to each rule's output side: rule index i uses marker `getMarkerString(i+1)` (since `@0@` is reserved for epsilon). Build `marker_symbols` = {`@1@`,...,`@N@`} and `marker_substitutions` mapping each to internal epsilon (applied at the end).
> Set up a tokenizer with `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`, `@LM2@`, `@RM2@`, `@TMPM@`, `.#.`. Let `type` = type of the first rule's first mapping's first transducer. Build bracket transducers for `@LM@`,`@RM@`,`@LM2@`,`@RM2@`,`@TMPM@`. Build `identityPair` and `identity` with `marker_symbols` in the alphabet (so unknowns/identities are not expanded to markers). Build `identityExpanded` = identity-pair with all markers (`@LM@`,`@RM@`,`@LM2@`,`@RM2@`,`@TMPM@` and `marker_symbols`) in alphabet. Build `removeHash` = `identityWithoutBoundary . .#. . identityWithoutBoundary` (identity star with `.#.` and marker symbols in alphabet).
> First pass over rules: copy each rule, `encodeFlags()`, build its combined `mapping` from its mapping pairs by cross-producting each pair's `.first` with (`.second` concatenated with that rule's marker), inserting marker_symbols into both sides' alphabets first; subtract `removeHash` (non-harmonizing) and remove `.#.` per pair (j==0 assigns, else disjunct+optimize). Track `noContexts`: set false if any rule has a single non-epsilon context. Handle ?->x edge case as in single bracketedReplace. Insert `@LM@`,`@RM@`,`@TMPM@` into mapping alphabet; build `mappingWithBrackets` = `@LM@ . mapping . @RM@`. If not optional, insert `@LM2@`/`@RM2@`, build `@LM2@ . (mapping input-project) . @RM2@`, disjunct into `mappingWithBrackets`. Disjunct `mappingWithBrackets` into `identityExpanded` and push it onto `mappingWithBracketsVector`.
> Repeat-star `identityExpanded` and optimize. If `noContexts`: remove `@TMPM@`, substitute markers to epsilon, remove marker_symbols, return `identityExpanded`. If `ruleVector.size() != mappingWithBracketsVector.size()`, throw TransducerTypeMismatchException ("Vector sizes don't match").
> Second pass: for each rule i, copy and encodeFlags; build `mappingWithBracketsAndTmpBoundary` = `@TMPM@ . mappingWithBracketsVector[i] . @TMPM@`; build `bracketedReplaceTmp` = `identityExpanded . that . identityExpanded`, zero weights, disjunct into accumulating `bracketedReplace`. Take the rule's contexts; if `replType != REPL_UP`, for every context pair freely insert (non-harmonizing) each marker symbol whose `getMarkerNumber` != i into both `.first` and `.second` (so markers other rules may emit are allowed through output-side contexts). Call `expandContextsWithMapping(cont, mappingWithBracketsAndTmpBoundary, identityExpanded, replType, optional)`, zero weights, disjunct into `unionContextReplace`, optimize.
> Compute `replaceWithoutContexts` = `bracketedReplace` minus `unionContextReplace`, optimized; substitute `@TMPM@` to epsilon, remove `@TMPM@` from alphabet, optimize; remove `@TMPM@` from `identityExpanded`'s alphabet. Final negation: `uncondidtionalTr` = `identityExpanded` minus `replaceWithoutContexts`, optimized. Substitute marker symbols to epsilon, remove them from alphabet, and return `uncondidtionalTr`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.remove-b2-constraint-fn]
> HfstTransducer removeB2Constraint( const HfstTransducer &t )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.remove-b2-constraint-fn]
> Removes paths containing the secondary brackets `@LM2@`/`@RM2@` (`B2`). Set up a tokenizer with `@_EPSILON_SYMBOL_@`, `@LM2@`, `@RM2@`. Let `type = t.get_type()`. Build `identityStar` = identity-pair star, `B = @LM2@ | @RM2@`.
> Assemble `Constraint` = `identityStar . B . identityStar`, optimized (`?* B2 ?*`). Compute `retval = constraintComposition(t, Constraint)`. Then remove `@LM2@` and `@RM2@` from `retval`'s alphabet, and return `retval`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.remove-markers-fn]
> HfstTransducer removeMarkers( const HfstTransducer &tr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.remove-markers-fn]
> Strips the `@LM@`/`@RM@` bracket markers and decodes flag diacritics. Copy `tr` into `retval`. Substitute the identity pair (`@LM@`,`@LM@`) with (epsilon, epsilon) and optimize; substitute (`@RM@`,`@RM@`) with (epsilon, epsilon) and optimize. Remove `@LM@` and `@RM@` from `retval`'s alphabet. Optimize.
> Then set `retval = decodeFlagDiacritics(retval)` (turns the `$...$` fake flags back into real `@...@` flag diacritics) and return `retval`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-arrow]
> enum ReplaceArrow {
>   E_REPLACE_RIGHT;
>   E_OPTIONAL_REPLACE_RIGHT;
>   E_REPLACE_LEFT;
>   E_OPTIONAL_REPLACE_LEFT;
>   E_REPLACE_RIGHT_MARKUP;
>   E_RTL_LONGEST_MATCH;
>   E_RTL_SHORTEST_MATCH;
>   E_LTR_LONGEST_MATCH;
>   E_LTR_SHORTEST_MATCH;
> }

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-epenthesis-fn]
> HfstTransducer replace_epenthesis( const Rule &rule, bool optional)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-epenthesis-fn]
> Thin wrapper for a single `Rule`: simply returns `replace(rule, optional)` (the single-rule replace). It exists only as a named alias for epenthesis replace; it performs no extra processing. (The overload taking a rule vector likewise just returns `replace(ruleVector, optional)`.)

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-fn]
> HfstTransducer replace( const std::vector<Rule> &ruleVector, bool optional)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-fn]
> Top-level replace for a vector of (possibly parallel) rules. If `ruleVector.size() == 1`, set `retval = bracketedReplace(ruleVector[0], optional)`; otherwise `retval = parallelBracketedReplace(ruleVector, optional)`.
> Then apply post-processing in order: `retval = noRepetitionConstraint(retval)` (no more than one epsilon repetition in a row, for epenthesis rules); `retval = applyBoundaryMark(retval)` (handles the `.#.` boundary symbol; must run before mostBracketsStarConstraint). If `!optional`, `retval = mostBracketsStarConstraint(retval)`. Then `retval = removeB2Constraint(retval)`, then `retval = removeMarkers(retval)`. Return `retval`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-left-fn]
> HfstTransducer replace_left( const std::vector<Rule> &ruleVector, bool optional)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-left-fn]
> Parallel left-replace: replaces on the lower (output) side rather than the upper side. For each rule in `ruleVector`, build a new rule whose mapping pairs are the original pairs with `.first` and `.second` swapped (each `(first, second)` becomes `(second, first)`), keeping the same context and replType; collect these into `leftRuleVector`.
> Call `replace(leftRuleVector, optional)` to get `retval`, then `retval.invert().optimize()` to restore the original input/output orientation, and return `retval`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-longest-match-fn]
> HfstTransducer replace_leftmost_longest_match( const std::vector<Rule> &ruleVector )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-longest-match-fn]
> Leftmost-longest-match replace for a rule vector. Build `uncondidtionalTr`: if one rule, `bracketedReplace(ruleVector[0], true)` (always optional=true here); else `parallelBracketedReplace(ruleVector, true)`.
> Apply `uncondidtionalTr = noRepetitionConstraint(uncondidtionalTr)` (before the leftmost constraint). Then `retval = leftMostConstraint(uncondidtionalTr)`; `retval = oneBetterthanNoneConstraint(retval)` (drop empty strings); `retval = longestMatchLeftMostConstraint(retval)`.
> Then `retval = removeB2Constraint(retval)` (remove `@LM2@`/`@RM2@`); `retval = removeMarkers(retval)`; `retval = applyBoundaryMark(retval)`. Return `retval`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-shortest-match-fn]
> HfstTransducer replace_leftmost_shortest_match(const std::vector<Rule> &ruleVector )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-shortest-match-fn]
> Leftmost-shortest-match replace for a rule vector. Build `uncondidtionalTr`: if one rule, `bracketedReplace(ruleVector[0], true)`; else `parallelBracketedReplace(ruleVector, true)`.
> Apply `uncondidtionalTr = noRepetitionConstraint(uncondidtionalTr)`. Then `retval = leftMostConstraint(uncondidtionalTr)`; `retval = oneBetterthanNoneConstraint(retval)`; `retval = shortestMatchLeftMostConstraint(retval)`.
> Then `retval = removeB2Constraint(retval)`; `retval = removeMarkers(retval)`; `retval = applyBoundaryMark(retval)`. Return `retval`. (Identical to the leftmost-longest variant except it uses the shortest-match constraint.)

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-longest-match-fn]
> HfstTransducer replace_rightmost_longest_match( const std::vector<Rule> &ruleVector )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-longest-match-fn]
> Rightmost-longest-match replace for a rule vector. Build `uncondidtionalTr`: if one rule, `bracketedReplace(ruleVector[0], true)`; else `parallelBracketedReplace(ruleVector, true)`.
> Then `retval = rightMostConstraint(uncondidtionalTr)`; `retval = longestMatchRightMostConstraint(retval)`; `retval = noRepetitionConstraint(retval)` (note: applied after the match constraints here, not before as in the leftmost variants); `retval = removeB2Constraint(retval)`; `retval = removeMarkers(retval)`; `retval = applyBoundaryMark(retval)`. Return `retval`. Note there is no oneBetterthanNoneConstraint step in the rightmost path.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-shortest-match-fn]
> HfstTransducer replace_rightmost_shortest_match( const std::vector<Rule> &ruleVector )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-shortest-match-fn]
> Rightmost-shortest-match replace for a rule vector. Build `uncondidtionalTr`: if `ruleVector.size() == 1`, `bracketedReplace(ruleVector[0], true)`; else `parallelBracketedReplace(ruleVector, true)`.
> Then `retval = rightMostConstraint(uncondidtionalTr)`; `retval = shortestMatchRightMostConstraint(retval)`; `retval = noRepetitionConstraint(retval)` (applied after the match constraints, as in the rightmost-longest variant); `retval = removeB2Constraint(retval)`; `retval = removeMarkers(retval)`; `retval = applyBoundaryMark(retval)`. Return `retval`. There is no oneBetterthanNoneConstraint step in the rightmost path.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-type]
> enum ReplaceType {
>   REPL_UP;
>   REPL_DOWN;
>   REPL_RIGHT;
>   REPL_LEFT;
> }

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.restriction-fn]
> HfstTransducer restriction( const HfstTransducer &_center, const HfstTransducerPairVector &context)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.restriction-fn]
> Implements the restriction operator `C => L1 _ R1 , L2 _ R2 , ...` (center C may only occur in one of the listed contexts), using a marker symbol `@_D_@` to delimit center boundaries. First verify `_center` is an automaton: build input-projection and output-projection copies and compare each to `_center`; if either differs throw TransducersAreNotAutomataException with message "HfstXeroxRules::restriction".
> Let `type = _center.get_type()`. Tokenizer multichar symbols: `@_D_@`, `@_EPSILON_SYMBOL_@`. Build `mark` = `@_D_@` transducer, `epsilon` = epsilon transducer. Build `identity` = identity-pair repeated star. Build `universalWithoutD` = `identity` with `@_D_@` inserted into its alphabet (so `@_D_@` is treated as a known symbol but not matched by `?`), and `universalWithoutDStar` = that repeated star (this is `U*`).
> Build `noDUpper` (NODU = `[U | 0:@_D_@]*`): epsilon-to-`@_D_@` disjunct `universalWithoutD`, repeat star, optimize. Build `noDLower` (NODL = `[U | @_D_@:0]*`): `@_D_@`-to-epsilon disjunct `universalWithoutD`, repeat star, optimize.
> Build `center` = copy of `_center` with `@_D_@` inserted into its alphabet. Build `centerMarked` (CEN1 = `[U* @_D_@ CENTER @_D_@ U*]`) = `universalWithoutDStar . mark . center . mark . universalWithoutDStar`, optimized.
> Build `contextMarked` by iterating contexts: for each context i, copy `context[i].first` into `lefContext` and `context[i].second` into `rightContext`, inserting `@_D_@` into each one's alphabet; build `RES` = `universalWithoutDStar . lefContext . mark . universalWithoutDStar . mark . rightContext . universalWithoutDStar`, optimized (`[U* L @_D_@ U* @_D_@ R U*]`). For i==0 assign to `contextMarked`, else disjunct and optimize.
> Build `centerMinusCtx` = `centerMarked` minus `contextMarked`, optimized. Build `tmp` = `noDUpper . compose centerMinusCtx . compose noDLower`, optimized (`NODU .o. (CEN1 - RES) .o. NODL`). Build `retval` = `universalWithoutDStar` minus `tmp`, optimized (`U* - tmp`). Remove `@_D_@` from `retval`'s alphabet. Finally `retval = applyBoundaryMark(retval)` and return `retval`.

> PORT NOTE (flag-complement.audit, follow-up to hfst/hfst#349): `restriction`,
> `before`, and `after` build their `U*`/`[?* X ?* Y ?*]` universes straight from
> `identity_pair()` and subtract WITHOUT first encoding flag diacritics, so — as
> in upstream C++ — a flag diacritic inside a restriction/before/after context is
> erased by subtract harmonization (the flag-swallowing shape fixed for XRE `~`).
> This is DEFERRED (kept 1:1 with upstream): the replace-rule family already
> guards itself by calling `Rule::encode_flags()` (flags become ordinary `$...$`
> symbols before any subtract), flags inside a bare restriction context are a
> genuinely unusual Xerox construction with no known consumer, and these
> operators are a heavily-spec'd 1:1 port with their own test suite
> (`HfstXeroxRulesTest.md` / `test_xerox_rules.rs`). The flag-free common path is
> locked by `test_flag_complement.rs::deferral_xerox_restriction_flag_free_baseline`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.right-most-constraint-fn]
> HfstTransducer rightMostConstraint( const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.right-most-constraint-fn]
> Builds the right-most constraint `[B:0|0:B|?-B]* [I-B]+ >:0 ?*` and applies it. Set up a tokenizer with `@_EPSILON_SYMBOL_@`, `@_UNKNOWN_SYMBOL_@`, `@LM@`, `@RM@`. Let `type = uncondidtionalTr.get_type()`.
> Build `identity` = identity-pair repeated star. Build `rightPart` via `constraintsRightPart(type)` (`[B:0|0:B|?-B]*`). Build `B = @LM@ | @RM@`, optimized. Build `bracketsToEpsilonStar` = `B` cross-product epsilon, optimize, repeat star, optimize (`[B:0]*`, computed but not used in the final assembly). Build `identityPairMinusBrackets` = `identityPair - B`, optimized; and `identityPairMinusBracketsPlus` = its plus-repetition (`[I-B]+`) and `identityPairMinusBracketsStar` = its star-repetition (also unused). Build `RightBracketToEpsilon` = `@RM@:0`.
> Assemble `Constraint` = `rightPart . identityPairMinusBracketsPlus . RightBracketToEpsilon . identity`, optimized (i.e. `[B:0|0:B|?-B]* [I-B]+ >:0 ?*`).
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule]
> class Rule {
>   HfstTransducerPairVector mapping;
>   HfstTransducerPairVector context;
>   ReplaceType replType;
> }

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.encode-flags-fn]
> void Rule::encodeFlags()

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.encode-flags-fn]
> Mutates the Rule in place, encoding flag diacritics in both its mappings and contexts. Copy `this->mapping` into `tmpM`; for each mapping pair i, set `tmpM[i].first = encodeFlagDiacritics(tmpM[i].first)` and `tmpM[i].second = encodeFlagDiacritics(tmpM[i].second)`. Copy `this->context` into `tmpC`; for each context pair i, set `tmpC[i].first = encodeFlagDiacritics(tmpC[i].first)` and `tmpC[i].second = encodeFlagDiacritics(tmpC[i].second)`. Then assign `this->mapping = tmpM` and `this->context = tmpC`. No return value.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-context-fn]
> HfstTransducerPairVector Rule::get_context() const

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-context-fn]
> Trivial const getter: returns a copy of the Rule's `context` member (an HfstTransducerPairVector).

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-mapping-fn]
> HfstTransducerPairVector Rule::get_mapping() const

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-mapping-fn]
> Trivial const getter: returns a copy of the Rule's `mapping` member (an HfstTransducerPairVector).

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.get-repl-type-fn]
> ReplaceType Rule::get_replType() const

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.get-repl-type-fn]
> Trivial const getter: returns the Rule's `replType` member (a ReplaceType enum value).

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.rule.rule-fn]
> Rule::Rule ( const HfstTransducerPairVector &mappingPairVector,

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.rule.rule-fn]
> Constructor from `mappingPairVector`, `a_context`, and `a_replType`. First validate type consistency: let `type = mappingPairVector[0].first.get_type()`. For each mapping pair i, if `mappingPairVector[i].first.get_type() != type` or `mappingPairVector[i].second.get_type() != type`, throw TransducerTypeMismatchException with message "Rule mapping". For each context pair j, if `a_context[j].first.get_type() != type` or `a_context[j].second.get_type() != type`, throw TransducerTypeMismatchException with message "Rule context".
> Then assign members: `mapping = mappingPairVector`, `context = a_context`, `replType = a_replType`. (No flag encoding is performed here.)
> Note: there is also a copy constructor that sets `mapping/context/replType` from the source rule's getters, and a default (SWIG) constructor that builds a single epsilon/epsilon context pair (using a tokenizer with `@_EPSILON_SYMBOL_@` and TROPICAL_OPENFST_TYPE) and sets `replType = REPL_UP`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.shortest-match-left-most-constraint-fn]
> HfstTransducer shortestMatchLeftMostConstraint( const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.shortest-match-left-most-constraint-fn]
> Builds the shortest-match left-most constraint `?* < [?-B]+ >:0 [middle] [B:0|0:B|?-B]*` and applies it (intended to be composed onto a left-most transducer). Tokenizer multichar symbols: `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`. Let `type = uncondidtionalTr.get_type()`.
> Build `identity` = identity-pair star, `rightPart` via `constraintsRightPart(type)` (`[B:0|0:B|?-B]*`), `B = @LM@ | @RM@`, `identityPairMinusBrackets = identityPair - B`, and `identityPairMinusBracketsPlus` (`[?-B]+`). Build the bracket-edit transducers `RightBracketToEpsilon` (`>:0`), `epsilonToRightBracket` (`0:>`), `LeftBracketToEpsilon` (`<:0`), `epsilonToLeftBracket` (`0:<`).
> Build `nonClosingBracketInsertion` = `epsilonToLeftBracket | LeftBracketToEpsilon | RightBracketToEpsilon | B`, optimized (`0:< | <:0 | >:0 | B`), then concatenate `identityPairMinusBracketsPlus` (`[0:<|<:0|>:0|B] [?-B]+`), optimized. Build `middlePart` = `identityPairMinusBrackets | nonClosingBracketInsertion`, optimized.
> Assemble `Constraint` = `identity . leftBracket . identityPairMinusBracketsPlus . RightBracketToEpsilon . middlePart`, optimized, then `. rightPart`, optimized (i.e. `?* < [?-B]+ >:0 [middle] [B:0|0:B|?-B]*`).
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.shortest-match-right-most-constraint-fn]
> HfstTransducer shortestMatchRightMostConstraint( const HfstTransducer &uncondidtionalTr )

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.shortest-match-right-most-constraint-fn]
> Builds the shortest-match right-most constraint `[B:0|0:B|?-B]* [middle] <:0 [?-B]+ > ?*` and applies it (intended to be composed onto a right-most transducer). Tokenizer multichar symbols: `@_EPSILON_SYMBOL_@`, `@LM@`, `@RM@`. Let `type = uncondidtionalTr.get_type()`.
> Build `identity` = identity-pair star, `rightPart` via `constraintsRightPart(type)` (`[B:0|0:B|?-B]*`), `B = @LM@ | @RM@`, `identityPairMinusBrackets = identityPair - B`, and `identityPairMinusBracketsPlus` (`[?-B]+`). Build bracket-edit transducers `RightBracketToEpsilon` (`>:0`), `epsilonToRightBracket` (`0:>`), `LeftBracketToEpsilon` (`<:0`), `epsilonToLeftBracket` (`0:<`).
> Build `nonClosingBracketInsertionTmp` = `epsilonToRightBracket | RightBracketToEpsilon | LeftBracketToEpsilon | B`, optimized (`0:> | >:0 | <:0 | B`). Build `nonClosingBracketInsertion` = `identityPairMinusBracketsPlus . nonClosingBracketInsertionTmp`, optimized (`[?-B]+ [0:>|>:0|<:0|B]`). Build `middlePart` = `identityPairMinusBrackets | nonClosingBracketInsertion`, optimized.
> Assemble `Constraint` = `rightPart . middlePart . LeftBracketToEpsilon . identityPairMinusBracketsPlus . rightBracket . identity`, optimized (i.e. `[B:0|0:B|?-B]* [middle] <:0 [?-B]+ > ?*`).
> Return `constraintComposition(uncondidtionalTr, Constraint)`.

> [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.zero-weight-fn]
> float zero_weight(float f)

> [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.zero-weight-fn]
> Trivial weight-mapping function: ignores its argument `f` (explicitly cast to void) and always returns `0`. Used as the callback passed to weight-transforming operations to set all transition weights to zero.

> [spec:hfst:def:hfst-xerox-rules.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-xerox-rules.main-fn]
> Compiled only under `MAIN_TEST`; this is the unit-test driver, not part of the library API. Print "Unit tests for <file>:" to stdout. Define `types[] = {SFST_TYPE, TROPICAL_OPENFST_TYPE, FOMA_TYPE}` (NUMBER_OF_TYPES=3). For each type index i from 0 to 2: skip (continue) if `HfstTransducer::is_implementation_type_available(types[i])` is false; otherwise run the full suite of replace tests for that type (test1, test1b, test1c, test1d, test2a/b/c, test3a-d, test4a-c, test6a-c, test7a-h, test9a/b, test10a/b — several other tests are commented out) followed by the restriction tests (restriction_test1, 1a, 1b, 2, 3, 3a, 3b, 3c, 4, 5, 5a, 6, 7, 8) and before_test1. After all types, print "ok" to stdout and return 0.

