# tools/src/hfst-substitute.cc

> [spec:hfst:def:hfst-substitute.from-arc-fn]
> hfst::StringPair from_arc(from_label, from_label)

> [spec:hfst:sem:hfst-substitute.from-arc-fn]
> When the source is a plain label (not a colon pair) and the target is a
> replacement transducer, the label is promoted to an identity arc by forming
> the StringPair (from_label, from_label) — i.e. the same symbol on both the
> input and output side. That pair is then handed to the pair-with-transducer
> substitution so every arc whose input and output both equal from_label is
> replaced by a copy of the target transducer.

> [spec:hfst:def:hfst-substitute.hfst-symbol-pair-substitutions]
> typedef std::map<StringPair, StringPair> HfstSymbolPairSubstitutions

> [spec:hfst:def:hfst-substitute.hfst-symbol-substitutions]
> typedef std::map<String, String> HfstSymbolSubstitutions

> [spec:hfst:def:hfst-substitute.label-to-stringpair-fn]
> static StringPair *

> [spec:hfst:sem:hfst-substitute.label-to-stringpair-fn]
> Parses an arc label of the form 'first:second' into a StringPair, or returns
> nothing if the label is not a pair. It scans for the separating colon as
> follows. Find the first ':'. While a candidate colon exists: if it is the very
> first character, look for the next ':' after it (a leading colon cannot
> separate); if it is the last character, give up (no second field); if the
> character immediately before it is a backslash, then — only when there is at
> least one character before that backslash — inspect the character two places
> back: a double backslash ('\\:') means the colon is a real separator (stop),
> otherwise the colon is escaped, so look for the next ':' after it; in any other
> case the colon is the separator (stop). (When the backslash is itself the very
> first character the candidate is left unchanged, matching the source.) After
> the scan, a pair is produced only when the chosen colon lies strictly inside
> the string (some character before it and after it); otherwise nothing is
> returned. The first field is the substring before the colon, the second the
> substring after it. Each field that equals "@0@" is rewritten to the internal
> epsilon symbol before the pair is returned.

> [spec:hfst:def:hfst-substitute.main-fn]
> int

> [spec:hfst:sem:hfst-substitute.main-fn]
> Entry point. Sets the program name (version "0.1", wiki "HfstSubstitute"),
> calls parse_options, and returns its status if it is not EXIT_CONTINUE.
> Otherwise it flushes/closes the stdio input and output buffers (the tool works
> with HFST streams), prints a verbose note about the source and destination
> file names, and — when a replacement file (-F) was given — allocates the empty
> label and pair substitution maps. It then opens the input HFST stream (the
> named input file, or standard input). If the stream is in optimized-lookup
> format it reports failure (EXIT_FAILURE); otherwise it runs process_stream on
> the input and returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-substitute.parse-options-fn]
> int

> [spec:hfst:sem:hfst-substitute.parse-options-fn]
> Parses the command line. After honouring HFST_OPTIONS from the environment, it
> loops over getopt_long with the common and unary option tables plus the tool's
> own short string "f:F:t:T:R9C" and long options. The tool-specific cases are:
> -f/--from-label sets the source label (rewriting "@0@" to internal epsilon),
> derives from_pair via label_to_stringpair, and errors out if the argument is
> empty; -F/--from-file records the replacement file name and opens it for
> reading (returning EXIT_FAILURE if the open fails); -t/--to-label sets the
> target label (same "@0@" handling and emptiness check) and derives to_pair;
> -T/--to-transducer records the target transducer file name and verifies it can
> be opened (open then immediately close, EXIT_FAILURE on failure); -R/--in-order
> keeps the order of the replacements read from a file; -9/--compose enables
> delayed composed substitution; -C/--do-not-convert forbids transducer-type
> conversion. After the loop it requires that a source was given (-f or -F) and
> that a target was given (-t, -T, or -F), erroring out otherwise. It finishes
> with the common and unary parameter checks and returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-substitute.perform-delayed-fn]
> static void

> [spec:hfst:sem:hfst-substitute.perform-delayed-fn]
> Finalises and applies the accumulated delayed (composed) substitution. It
> builds an identity arc transducer (internal_identity:internal_identity) of the
> transducer's type as sigmaMinusSubs, projects the substitution net to its input
> side, and subtracts that input language from sigmaMinusSubs so the result
> covers every symbol NOT touched by a substitution. It disjuncts that remainder
> back into the substitution net and applies repeat-star, yielding a net that
> either rewrites a substituted symbol or copies any other symbol, any number of
> times. It composes the working transducer with this net on the right and
> minimises, then inverts the net and composes it on the left (assigning the
> composed result back into the working transducer) and minimises again.

> [spec:hfst:def:hfst-substitute.print-usage-fn]
> void

> [spec:hfst:sem:hfst-substitute.print-usage-fn]
> Prints the help text to the message stream: a usage line "Usage: PROG
> [OPTIONS...] [INFILE]" with the heading "Relabel transducer arcs", then the
> common and common-unary option blocks, the relabeling options (-f, -t, -T, -F,
> -R), the input option (-C), and the transient optimisation option (-9),
> followed by the common unary parameter instructions, an explanation of what a
> LABEL/TFILE/LABELFILE is, three worked examples (substituting a symbol, a pair,
> and a pair with a transducer), the bug-report footer and the more-info footer.

> [spec:hfst:def:hfst-substitute.process-stream-fn]
> int

> [spec:hfst:sem:hfst-substitute.process-stream-fn]
> Reads transducers from the input stream and writes their relabeled versions to
> the output stream. If a target transducer file (-T) was given it first reads
> that transducer; if its type differs from the input's, then — unless conversion
> was forbidden (-C, in which case it errors out) — it chooses the output type
> per conversion_type (former, latter, or former-with-possible-loss), warns, and
> converts the target transducer to that type. The output type is otherwise the
> input type. It opens the output stream (named file or stdout) for that type.
> For each input transducer it announces progress, creates an empty delayed
> substitution net of the transducer's type, and performs substitutions: when a
> replacement file was given it reads it line by line (skipping blank lines and
> '#' comments, requiring a tab per line, erroring on empty fields), and for each
> pair/pair or label/label entry either records it into the pending substitution
> map (default) or applies it immediately when --in-order is set; after the file
> it applies the collected label and pair maps in one shot. Without a file it
> applies the single -f/-t/-T substitution directly. Substitution that is not
> supported for the transducer's native type is caught and retried after
> converting to the internal basic format (warned once). If a fallback occurred
> the basic result is converted back to the transducer's type; otherwise, if any
> delayed (composed) substitution was queued, perform_delayed is run. The output
> transducer is then named and given a formula reflecting the operation
> (substitute-from-FILE, substitute-FROM-with-TO, or
> substitute-FROM-with-net-TFILE), and written to the output stream.
