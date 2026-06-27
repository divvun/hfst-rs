# tools/src/hfst-tool-metadata.cc

> [spec:hfst:def:hfst-tool-metadata.hfst-get-name-fn]
> char*

> [spec:hfst:sem:hfst-tool-metadata.hfst-get-name-fn]
> Returns a display name for transducer 'arg', falling back to its source
> filename. Read 'arg's 'name' property. If it is non-empty, return a copy of
> that name. Otherwise return a copy of 'filename'. (The C version returned a
> 'strdup'd C string; the Rust port returns an owned String.)

> [spec:hfst:def:hfst-tool-metadata.hfst-set-commandline-def-fn]
> void hfst_set_commandline_def(hfst::HfstTransducer& dest,

> [spec:hfst:sem:hfst-tool-metadata.hfst-set-commandline-def-fn]
> Binary form. Builds the 'commandline-definition' property of 'dest' from the
> definitions of two operands 'lhs'/'rhs' plus the current process argv. Steps:
> (1) Start 'cmdline' from 'lhs's 'commandline-definition' property. (2) If that
> string is non-empty, append "&& ". (3) If 'rhs's 'commandline-definition'
> property is non-empty, append it. (4) If 'cmdline' is now non-empty, append
> "; ". (5) Append the basename of argv[0] (the file component after the last
> '/'). (6) For 'i' from 1 through 'argc' inclusive (argc == argv.len(); this
> off-by-one mirrors the C source): if argv[i] equals "-v" or "--verbose", skip
> it; otherwise, if it equals "-o" or "--output", set a flag 'o = true'; then
> append argv[i] to 'cmdline' (no separator inserted between arguments). (7) If
> 'o' was never set, append " > ??? ". (8) Store 'cmdline' into 'dest's
> 'commandline-definition' property. The unary form is the same but seeds
> 'cmdline' from a single 'src' operand's 'commandline-definition' (appending
> "; " if non-empty), and the string form seeds from an empty string.

> [spec:hfst:def:hfst-tool-metadata.hfst-set-formula-fn]
> void

> [spec:hfst:sem:hfst-tool-metadata.hfst-set-formula-fn]
> Binary form. Composes a 'formulaic-definition' for 'dest' from operator 'op'
> and the formulae of operands 'lhs'/'rhs', using "." as a placeholder for a
> missing operand formula. Read each operand's 'formulaic-definition' property.
> Four cases: (1) both non-empty -> "<lhs> <op> <rhs>"; (2) lhs empty, rhs
> non-empty -> ". <op> <rhs>"; (3) lhs non-empty, rhs empty -> "<lhs> <op> .";
> (4) both empty -> ". <op> .". Pass the result through the formula
> maybe-truncate helper, which stores it in 'dest's 'formulaic-definition'
> property. The unary form yields "<op> <src-formula>", or "<op> ." when the
> source has no formula. The string form inspects the first byte of 'src': if its
> signed-char value is in 1..127 (printable ASCII) it uses "<op> <first-char>";
> otherwise (UTF-8 multibyte / non-ASCII first byte) it uses "<op> U8".

> [spec:hfst:def:hfst-tool-metadata.hfst-set-formula-maybe-truncate-fn]
> void

> [spec:hfst:sem:hfst-tool-metadata.hfst-set-formula-maybe-truncate-fn]
> Sets the 'formulaic-definition' property of 'dest' to string 's', truncating
> oversized formulae. If 's' is longer than 1024 bytes, store the literal
> "TRUNC" instead; otherwise store 's' verbatim.

> [spec:hfst:def:hfst-tool-metadata.hfst-set-name-fn]
> void

> [spec:hfst:sem:hfst-tool-metadata.hfst-set-name-fn]
> Binary form. Composes a 'name' for 'dest' from operator 'op' and the names of
> operands 'lhs'/'rhs', using the literal UNNAMED for an unnamed operand. Read
> each operand's 'name' property. Four cases: (1) both named ->
> "<op>(<lhs>, <rhs>)"; (2) lhs unnamed, rhs named -> "<op>(UNNAMED, <rhs>)";
> (3) lhs named, rhs unnamed -> "<op>(<lhs>, UNNAMED)"; (4) both unnamed ->
> "<op>(UNNAMED, UNNAMED)". (A final unreachable else raises a logic error,
> ported as a panic.) Pass the result through the name maybe-truncate helper. The
> unary form yields "<op>(<src-name>)", or "<op>(UNNAMED)" when the source is
> unnamed. The string form yields "<op>(<src>)".

> [spec:hfst:def:hfst-tool-metadata.hfst-set-name-maybe-truncate-fn]
> void

> [spec:hfst:sem:hfst-tool-metadata.hfst-set-name-maybe-truncate-fn]
> Sets the 'name' property of 'dest' to string 's', truncating oversized names.
> If 's' is longer than 1024 bytes, store "truncated(" + first 1000 bytes of 's'
> + "...)"; otherwise store 's' verbatim.
