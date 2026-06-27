# tools/src/hfst-program-options.cc, tools/src/hfst-program-options.h

> [spec:hfst:def:hfst-program-options.print-common-binary-program-options-fn]
> void

> [spec:hfst:sem:hfst-program-options.print-common-binary-program-options-fn]
> Writes the shared "Input/Output options" help block for two-input/one-output
> binary tools (compose, concatenate, conjunct, disjunct, ...) to the given
> FILE*. A single fprintf emits, each line terminated by '\n':
>   "Input/Output options:"
>   "  -1, --input1=INFILE1   Read first input transducer from INFILE1"
>   "  -2, --input2=INFILE2   Read second input transducer from INFILE2"
>   "  -C, --do-not-convert   Do not allow transducers to be converted into the same type"
>   "  -o, --output=OUTFILE   Write results to OUTFILE"
> No arguments are interpolated; the text is fixed. Returns nothing.

> [spec:hfst:def:hfst-program-options.print-common-binary-program-parameter-instructions-fn]
> void

> [spec:hfst:sem:hfst-program-options.print-common-binary-program-parameter-instructions-fn]
> Writes the parameter-usage notes for binary tools to the given FILE* using two
> consecutive fprintf calls. The first emits, each line '\n'-terminated:
>   "If OUTFILE, or either INFILE1 or INFILE2 is missing or -,"
>   "standard streams will be used."
>   "INFILE1, INFILE2, or both, must be specified."
>   "Format of result depends on format of INFILE1 and INFILE2;"
>   "both should have the same format."
> The second fprintf emits a leading blank line ('\n') followed by:
>   "The operation is applied pairwise for INFILE1 and INFILE2"
>   "that must have the same number of transducers."
>   "If INFILE2 has only one transducer, the operation is applied for"
>   "each transducer in INFILE1 keeping the second transducer constant."
> Splitting across two fprintf calls is preserved bug-for-bug; the rendered
> output is identical to a single concatenation. No arguments are interpolated.

> [spec:hfst:def:hfst-program-options.print-common-program-options-fn]
> void

> [spec:hfst:sem:hfst-program-options.print-common-program-options-fn]
> Writes the "Common options" help block shared by every hfst tool to the given
> FILE*. A single fprintf emits, each line '\n'-terminated:
>   "Common options:"
>   "  -h, --help             Print help message"
>   "  -V, --version          Print version info"
>   "  -v, --verbose          Print verbosely while processing"
>   "  -q, --quiet            Only print fatal erros and requested output"
>   "  -s, --silent           Alias of --quiet"
>   "      --colour[=WHEN]    Print in colour WHEN:"
>   "      --color[=WHEN]     always, never, auto (default)"
> The misspelling "erros" (for "errors") in the --quiet line is part of the
> verbatim text and is preserved bug-for-bug. No arguments are interpolated.

> [spec:hfst:def:hfst-program-options.print-common-unary-program-options-fn]
> void

> [spec:hfst:sem:hfst-program-options.print-common-unary-program-options-fn]
> Writes the shared "Input/Output options" help block for one-input/one-output
> unary tools (determinize, invert, minimize, project, reverse, ...) to the
> given FILE*. A single fprintf emits, each line '\n'-terminated:
>   "Input/Output options:"
>   "  -i, --input=INFILE     Read input transducer from INFILE"
>   "  -o, --output=OUTFILE   Write output transducer to OUTFILE"
> No arguments are interpolated. Returns nothing.

> [spec:hfst:def:hfst-program-options.print-common-unary-program-parameter-instructions-fn]
> void

> [spec:hfst:sem:hfst-program-options.print-common-unary-program-parameter-instructions-fn]
> Writes the parameter-usage notes for unary tools to the given FILE*. A single
> fprintf emits, each line '\n'-terminated:
>   "If OUTFILE or INFILE is missing or -, standard streams will be used."
>   "Format of result depends on format of INFILE"
> No arguments are interpolated. Returns nothing.

> [spec:hfst:def:hfst-program-options.print-common-unary-string-program-options-fn]
> void print_common_unary_string_program_options(FILE *file)

> [spec:hfst:sem:hfst-program-options.print-common-unary-string-program-options-fn]
> Declared in hfst-program-options.h (intended for one-transducer-to-text tools
> such as fst2txt and fst2strings) but never defined in any translation unit of
> the C sources. There is no body to port; it has no observable behaviour and
> writes nothing. Ported as an empty-body stub taking the FILE* and returning
> nothing, so that any caller links and runs as a no-op.

